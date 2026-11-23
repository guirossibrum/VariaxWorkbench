//! Product families. Each submodule is self-contained: its requests, its
//! procedures and its decoders. To add a family, add a module and a
//! [`Family`] entry; nothing else in the crate needs to know its messages.

pub mod podxt;
pub mod variax;

use crate::identity::Identity;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// First-generation Variax (300/500/600/700), identity family 0x0007.
    Variax,
    /// PODxt family, identity family 0x0003 (PODxt Live member 0x000a).
    Podxt,
}

impl Family {
    /// Which supported family sent this identity, if any. Only identities
    /// verified against a real device or the original editor are listed.
    pub fn from_identity(identity: &Identity) -> Option<Self> {
        if !identity.is_line6() {
            return None;
        }
        match identity.family {
            variax::IDENTITY_FAMILY => Some(Family::Variax),
            podxt::IDENTITY_FAMILY => Some(Family::Podxt),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Family::Variax => "Variax",
            Family::Podxt => "PODxt",
        }
    }

    pub fn member_name(self, member: u16) -> &'static str {
        match self {
            Family::Variax => variax::member_name(member),
            Family::Podxt => podxt::member_name(member),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::tests::{PODXT_LIVE, VARIAX_600};

    #[test]
    fn identities_map_to_families() {
        let variax = Identity::parse(&VARIAX_600).unwrap();
        let podxt = Identity::parse(&PODXT_LIVE).unwrap();
        assert_eq!(Family::from_identity(&variax), Some(Family::Variax));
        assert_eq!(Family::from_identity(&podxt), Some(Family::Podxt));
        assert_eq!(Family::Variax.member_name(variax.member), "Variax 300/600");
        assert_eq!(Family::Podxt.member_name(podxt.member), "PODxt Live");
    }
}
