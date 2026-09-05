use std::path::Path;

use kotoclip_nlp::{ProviderRouter, RoutedAnalysis, VibratoProvider};

pub struct UniDicRuntime {
    pub router: ProviderRouter,
}

impl UniDicRuntime {
    pub fn open(cwj: impl AsRef<Path>, csj: impl AsRef<Path>) -> Result<Self, String> {
        let written = VibratoProvider::open("unidic-cwj-202512", cwj).map_err(|error| error.to_string())?;
        let spoken = VibratoProvider::open("unidic-csj-202512", csj).map_err(|error| error.to_string())?;
        Ok(Self {
            router: ProviderRouter { written, spoken },
        })
    }

    pub fn analyze(&self, text: &str) -> RoutedAnalysis {
        self.router.analyze(text)
    }
}
