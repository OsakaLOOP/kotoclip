"""UniDic token 输入的自有模型骨架。

模型只依赖 token 特征，不调用 GiNZA、KWJA、Sudachi 或 Juman 的运行时代码。
"""

from __future__ import annotations

from typing import Mapping

try:
    import torch
    from torch import Tensor, nn
except ImportError:  # pragma: no cover - 导入工具可在无 torch 环境中使用
    torch = None  # type: ignore[assignment]
    Tensor = object  # type: ignore[misc,assignment]
    nn = object  # type: ignore[assignment]


def _require_torch() -> None:
    if torch is None:
        raise RuntimeError("training requires PyTorch; data validation remains stdlib-only")


if torch is not None:

    class TokenFeatureEncoder(nn.Module):
        """以 UniDic 类别特征组成的轻量 Transformer encoder。"""

        def __init__(self, *, vocab_size: int, hidden_size: int, layers: int, heads: int, dropout: float, max_positions: int = 256) -> None:
            super().__init__()
            self.lemma = nn.Embedding(vocab_size, hidden_size)
            self.surface = nn.Embedding(vocab_size, hidden_size)
            self.pos = nn.Embedding(64, hidden_size)
            self.conj = nn.Embedding(256, hidden_size)
            self.register = nn.Embedding(4, hidden_size)
            self.position = nn.Embedding(max_positions, hidden_size)
            layer = nn.TransformerEncoderLayer(
                d_model=hidden_size,
                nhead=heads,
                dim_feedforward=hidden_size * 4,
                dropout=dropout,
                batch_first=True,
                norm_first=True,
                activation="gelu",
            )
            self.encoder = nn.TransformerEncoder(layer, num_layers=layers)
            self.norm = nn.LayerNorm(hidden_size)

        def forward(self, features: Mapping[str, Tensor], padding_mask: Tensor | None = None) -> Tensor:
            positions = torch.arange(features["lemma_id"].size(1), device=features["lemma_id"].device).unsqueeze(0)
            hidden = (
                self.lemma(features["lemma_id"])
                + self.surface(features["surface_id"])
                + self.pos(features["pos_id"])
                + self.conj(features["conj_id"])
                + self.register(features["register_id"])
                + self.position(positions)
            )
            return self.norm(self.encoder(hidden, src_key_padding_mask=padding_mask))


    class StructureModel(nn.Module):
        """GiNZA 能力域：共享 encoder、边界 head、依存 head。"""

        def __init__(self, *, vocab_size: int = 32768, hidden_size: int = 256, layers: int = 6, heads: int = 8, dropout: float = 0.15, max_positions: int = 256) -> None:
            super().__init__()
            self.encoder = TokenFeatureEncoder(vocab_size=vocab_size, hidden_size=hidden_size, layers=layers, heads=heads, dropout=dropout, max_positions=max_positions)
            self.sentence = nn.Linear(hidden_size, 2)
            self.compound = nn.Linear(hidden_size, 3)
            self.bunsetsu = nn.Linear(hidden_size, 3)
            self.dependency_head = nn.Bilinear(hidden_size, hidden_size, 1)
            self.dependency_label = nn.Linear(hidden_size * 2, 32)

        def forward(self, features: Mapping[str, Tensor], padding_mask: Tensor | None = None) -> dict[str, Tensor]:
            hidden = self.encoder(features, padding_mask)
            source = hidden.unsqueeze(2).expand(-1, -1, hidden.size(1), -1)
            target = hidden.unsqueeze(1).expand(-1, hidden.size(1), -1, -1)
            return {
                "hidden": hidden,
                "sentence_logits": self.sentence(hidden),
                "compound_logits": self.compound(hidden),
                "bunsetsu_logits": self.bunsetsu(hidden),
                "dependency_logits": self.dependency_head(source, target).squeeze(-1),
                "dependency_label_logits": self.dependency_label(torch.cat([source, target], dim=-1)),
            }


    class SemanticModel(nn.Module):
        """KWJA 能力域：UniDic 词元级共享 encoder 与可独立关闭的任务 head。"""

        def __init__(self, *, vocab_size: int = 32768, hidden_size: int = 384, layers: int = 8, heads: int = 8, dropout: float = 0.15, max_positions: int = 256) -> None:
            super().__init__()
            self.encoder = TokenFeatureEncoder(vocab_size=vocab_size, hidden_size=hidden_size, layers=layers, heads=heads, dropout=dropout, max_positions=max_positions)
            self.heads = nn.ModuleDict(
                {
                    "pos": nn.Linear(hidden_size, 32),
                    "subpos": nn.Linear(hidden_size, 64),
                    "conjtype": nn.Linear(hidden_size, 128),
                    "conjform": nn.Linear(hidden_size, 256),
                    "reading": nn.Linear(hidden_size, 16384),
                    "ner": nn.Linear(hidden_size, 17),
                    "word_feature": nn.Linear(hidden_size, 8),
                    "base_phrase_feature": nn.Linear(hidden_size, 64),
                    "predicate": nn.Linear(hidden_size, 2),
                    "clause": nn.Linear(hidden_size, 3),
                }
            )
            self.argument = nn.Bilinear(hidden_size, hidden_size, 8)

        def forward(self, features: Mapping[str, Tensor], padding_mask: Tensor | None = None) -> dict[str, Tensor]:
            hidden = self.encoder(features, padding_mask)
            output = {name: head(hidden) for name, head in self.heads.items()}
            output["hidden"] = hidden
            source = hidden.unsqueeze(2).expand(-1, -1, hidden.size(1), -1)
            target = hidden.unsqueeze(1).expand(-1, hidden.size(1), -1, -1)
            output["argument_logits"] = self.argument(source, target)
            return output

else:

    class TokenFeatureEncoder:  # type: ignore[no-redef]
        def __init__(self, **_: object) -> None:
            _require_torch()

    class StructureModel(TokenFeatureEncoder):  # type: ignore[no-redef]
        pass

    class SemanticModel(TokenFeatureEncoder):  # type: ignore[no-redef]
        pass
