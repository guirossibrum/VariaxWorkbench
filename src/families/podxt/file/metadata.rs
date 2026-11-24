use super::{parameters::*, *};

// L6PatchMetaTagWrapper stores these fields in L6MetaTagSet (LIST/UNFO
// or legacy LIST/INFO). The tone's display name lives in PINF.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PodMetadata {
    Song,
    Guitarist,
    Band,
    Author,
    PickupType,
    PickupPosition,
    Style,
    Notes,
}

impl PodMetadata {
    pub const ALL: [Self; 8] = [
        Self::Song,
        Self::Guitarist,
        Self::Band,
        Self::Author,
        Self::PickupType,
        Self::PickupPosition,
        Self::Style,
        Self::Notes,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Song => "Song",
            Self::Guitarist => "Guitarist",
            Self::Band => "Band",
            Self::Author => "Author",
            Self::PickupType => "Pickup type",
            Self::PickupPosition => "Pickup position",
            Self::Style => "Style",
            Self::Notes => "Notes",
        }
    }

    fn tag(self) -> &'static [u8; 4] {
        match self {
            Self::Song => b"ISNG",
            Self::Guitarist => b"IGTR",
            Self::Band => b"IBND",
            Self::Author => b"IAUT",
            Self::PickupType => b"IPUS",
            Self::PickupPosition => b"IPUP",
            Self::Style => b"ISTL",
            Self::Notes => b"ICMT",
        }
    }
}

impl PodTone {
    fn metadata_container(&self) -> Result<Option<(preset::Chunk, bool)>, ToneError> {
        let chunks =
            preset::chunks(&self.bytes, 12..self.bytes.len()).map_err(ToneError::format)?;
        let mut found = chunks.into_iter().filter_map(|chunk| {
            if preset::container(&self.bytes, &chunk, b"UNFO") {
                Some((chunk, true))
            } else if preset::container(&self.bytes, &chunk, b"INFO") {
                Some((chunk, false))
            } else {
                None
            }
        });
        let first = found.next();
        if found.next().is_some() {
            return Err(ToneError::format("Ambiguous metadata containers"));
        }
        Ok(first)
    }

    fn metadata_field(
        &self,
        container: &preset::Chunk,
        key: PodMetadata,
    ) -> Result<Option<preset::Chunk>, ToneError> {
        let fields = preset::chunks(&self.bytes, container.data.start + 4..container.data.end)
            .map_err(ToneError::format)?;
        let mut matches = fields.into_iter().filter(|chunk| &chunk.id == key.tag());
        let first = matches.next();
        if matches.next().is_some() {
            return Err(ToneError::format("Ambiguous metadata field"));
        }
        Ok(first)
    }

    pub fn metadata(&self, key: PodMetadata) -> Result<Option<String>, ToneError> {
        let Some((container, unicode)) = self.metadata_container()? else {
            return Ok(None);
        };
        let Some(field) = self.metadata_field(&container, key)? else {
            return Ok(None);
        };
        let data = &self.bytes[field.data];
        if unicode {
            let (pairs, remainder) = data.as_chunks::<2>();
            if !remainder.is_empty() {
                return Err(ToneError::format("Invalid UTF-16 metadata length"));
            }
            let units: Vec<_> = pairs.iter().copied().map(u16::from_be_bytes).collect();
            String::from_utf16(&units)
                .map(Some)
                .map_err(|_| ToneError::format("Invalid UTF-16 metadata"))
        } else {
            std::str::from_utf8(data)
                .map(|value| Some(value.to_owned()))
                .map_err(|_| ToneError::format("Opaque legacy text encoding"))
        }
    }

    pub fn set_metadata(&mut self, key: PodMetadata, value: &str) -> Result<(), ToneError> {
        if value.contains('\0') || value.len() > TONE_BYTES_CEILING as usize / 2 {
            return Err(ToneError::format(
                "Metadata exceeds the text limit or contains NUL",
            ));
        }
        let old = self.metadata(key)?;
        if old.as_deref() == Some(value) || old.is_none() && value.is_empty() {
            return Ok(());
        }
        let current = self.metadata_container()?;
        let unicode = current.as_ref().is_none_or(|(_, unicode)| *unicode);
        let bytes = if unicode {
            value
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<_>>()
        } else if value.is_ascii() {
            value.as_bytes().to_vec()
        } else {
            return Err(ToneError::format(
                "Use ASCII for this legacy INFO container",
            ));
        };
        let mut candidate = self.bytes.clone();
        let leaf = iff_chunk(key.tag(), &bytes);
        if let Some((container, _)) = current {
            let range = self
                .metadata_field(&container, key)?
                .map_or(container.data.end..container.data.end, |field| {
                    field.data.start - 8..field.data.end + (field.data.len() & 1)
                });
            splice_iff(&mut candidate, range, leaf, &[4, container.data.start - 4])?;
        } else {
            let mut container = b"UNFO".to_vec();
            container.extend(leaf);
            let end = candidate.len();
            splice_iff(
                &mut candidate,
                end..end,
                iff_chunk(b"LIST", &container),
                &[4],
            )?;
        }
        *self = Self::parse(candidate)?;
        Ok(())
    }
}
