"""UniDic 底座 NLP 重训练基础设施。"""

from .contracts import ContractError, validate_document, validate_span
from .features import FeatureTokenizer, FeatureVocabulary

__all__ = ["ContractError", "validate_document", "validate_span", "FeatureTokenizer", "FeatureVocabulary"]
