//! Printer command languages that ThermalPrint can send straight to a printer, bypassing its
//! driver. Both send the same validated raster, so what prints matches the preview.
use crate::design::LabelDocument;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RawLanguage {
    /// Zebra Programming Language, for Zebra and ZPL-compatible printers.
    Zpl,
    /// TSC Printer Language, used by many low-cost 4 × 6 shipping label printers.
    Tspl,
}

impl RawLanguage {
    pub const ALL: [RawLanguage; 2] = [RawLanguage::Zpl, RawLanguage::Tspl];

    pub fn name(self) -> &'static str {
        match self {
            RawLanguage::Zpl => "ZPL",
            RawLanguage::Tspl => "TSPL",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            RawLanguage::Zpl => "zpl",
            RawLanguage::Tspl => "tspl",
        }
    }

    /// The language for an export file extension.
    pub fn from_extension(extension: &str) -> Option<RawLanguage> {
        Self::ALL.into_iter().find(|l| l.extension() == extension)
    }

    /// The design as printer commands, with the optional footer kept `edge_mm` from the edges.
    pub fn encode_for_print(
        self,
        doc: &LabelDocument,
        dpi: u32,
        footer: &crate::footer::Footer,
        edge_mm: f32,
    ) -> Result<Vec<u8>> {
        match self {
            RawLanguage::Zpl => crate::zpl::encode_design_for_print(doc, dpi, footer, edge_mm),
            RawLanguage::Tspl => crate::tspl::encode_design_for_print(doc, dpi, footer, edge_mm),
        }
    }

    /// The design as printer commands for an export file.
    pub fn encode(self, doc: &LabelDocument, dpi: u32) -> Result<Vec<u8>> {
        match self {
            RawLanguage::Zpl => crate::zpl::encode_design(doc, dpi),
            RawLanguage::Tspl => crate::tspl::encode_design(doc, dpi),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_round_trip() {
        for language in RawLanguage::ALL {
            assert_eq!(
                RawLanguage::from_extension(language.extension()),
                Some(language)
            );
        }
        assert_eq!(RawLanguage::from_extension("png"), None);
    }
}
