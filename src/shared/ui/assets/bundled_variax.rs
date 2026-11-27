//! Artwork keys used by the Variax editor. Provenance is in assets/ui/SOURCES.md.

macro_rules! bundled_pngs {
    (aliases { $($alias:literal => $file:literal,)* } files { $($key:literal,)* }) => {
        pub(super) fn png(relative: &str) -> Option<&'static [u8]> {
            match relative {
                $($alias => Some(include_bytes!(concat!("../../../../assets/ui/variax/", $file))),)*
                $($key => Some(include_bytes!(concat!("../../../../assets/ui/variax/", $key))),)*
                _ => None,
            }
        }

        #[cfg(test)]
        pub(super) const KEYS: &[(&str, &str)] = &[
            $(($alias, concat!("variax/", $file)),)*
            $(($key, concat!("variax/", $key)),)*
        ];
    };
}

bundled_pngs! {
    aliases {
        "bodies/strat.png" => "spank-body.png",
        "pickups/strat.png" => "spank-pickup.png",
    }
    files {
        "bodies/acoustic.png",
        "bodies/banjo.png",
        "bodies/casino.png",
        "bodies/countrygent.png",
        "bodies/danelectro.png",
        "bodies/dobro.png",
        "bodies/es175.png",
        "bodies/es335.png",
        "bodies/firebird.png",
        "bodies/lester.png",
        "bodies/lesterblack.png",
        "bodies/lestergoldtop.png",
        "bodies/lpjunior.png",
        "bodies/ricky.png",
        "bodies/sitar.png",
        "bodies/sparklejet.png",
        "bodies/special.png",
        "bodies/super400.png",
        "bodies/tele.png",
        "bodies/telecustom.png",
        "bodies/thinline.png",
        "bodies/tricone.png",
        "bodies/wireframe/banjo.png",
        "bodies/wireframe/casino.png",
        "bodies/wireframe/danelectro.png",
        "bodies/wireframe/dobro.png",
        "bodies/wireframe/es175.png",
        "bodies/wireframe/es335.png",
        "bodies/wireframe/firebird.png",
        "bodies/wireframe/gretsch6120.png",
        "bodies/wireframe/guildf212.png",
        "bodies/wireframe/j200.png",
        "bodies/wireframe/juniorsinglecut.png",
        "bodies/wireframe/lespaul.png",
        "bodies/wireframe/lpspecial.png",
        "bodies/wireframe/martin018.png",
        "bodies/wireframe/martind12_28.png",
        "bodies/wireframe/martind28.png",
        "bodies/wireframe/ric12.png",
        "bodies/wireframe/ric6.png",
        "bodies/wireframe/sitar.png",
        "bodies/wireframe/sparklejet.png",
        "bodies/wireframe/strat.png",
        "bodies/wireframe/super400.png",
        "bodies/wireframe/tele.png",
        "bodies/wireframe/thinline.png",
        "bodies/wireframe/tricone.png",
        "pickups/blackp90.png",
        "pickups/chromep90.png",
        "pickups/chromepaf_b.png",
        "pickups/chromepaf_n.png",
        "pickups/cremep90.png",
        "pickups/dearmond.png",
        "pickups/filtertron.png",
        "pickups/goldpaf_b.png",
        "pickups/goldpaf_n.png",
        "pickups/lipstick.png",
        "pickups/mini.png",
        "pickups/telebridge.png",
        "pickups/teleneck.png",
        "pickups/toaster.png",
        "bodies/buttons/banjo.png",
        "bodies/buttons/casino.png",
        "bodies/buttons/danelectro.png",
        "bodies/buttons/dobro.png",
        "bodies/buttons/es175.png",
        "bodies/buttons/es335.png",
        "bodies/buttons/firebird.png",
        "bodies/buttons/gretsch6120.png",
        "bodies/buttons/guildf212.png",
        "bodies/buttons/j200.png",
        "bodies/buttons/juniorsinglecut.png",
        "bodies/buttons/lespaul.png",
        "bodies/buttons/lpspecial.png",
        "bodies/buttons/martin018.png",
        "bodies/buttons/martind12_28.png",
        "bodies/buttons/martind28.png",
        "bodies/buttons/ric12.png",
        "bodies/buttons/ric6.png",
        "bodies/buttons/sitar.png",
        "bodies/buttons/sparklejet.png",
        "bodies/buttons/strat.png",
        "bodies/buttons/super400.png",
        "bodies/buttons/tele.png",
        "bodies/buttons/thinline.png",
        "bodies/buttons/tricone.png",
    }
}
