use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::entities::ArtifactDescriptor;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageFingerprint {
    pub value: String,
    pub inputs: Vec<String>,
}

impl StageFingerprint {
    pub fn from_inputs(inputs: impl IntoIterator<Item = String>) -> Self {
        let mut inputs = inputs.into_iter().collect::<Vec<_>>();
        inputs.sort();
        let mut hash = Sha256::new();
        for input in &inputs {
            hash.update(input.as_bytes());
            hash.update([0]);
        }
        Self {
            value: format!("sha256:{:x}", hash.finalize()),
            inputs,
        }
    }
}

pub trait StageArtifact: Serialize {
    fn descriptor(&self) -> &ArtifactDescriptor;
    fn fingerprint(&self) -> &StageFingerprint;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyStageArtifact {
    pub descriptor: ArtifactDescriptor,
    pub fingerprint: StageFingerprint,
}

impl StageArtifact for EmptyStageArtifact {
    fn descriptor(&self) -> &ArtifactDescriptor {
        &self.descriptor
    }

    fn fingerprint(&self) -> &StageFingerprint {
        &self.fingerprint
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_order_independent() {
        let a = StageFingerprint::from_inputs(["b".to_string(), "a".to_string()]);
        let b = StageFingerprint::from_inputs(["a".to_string(), "b".to_string()]);
        assert_eq!(a, b);
    }
}
