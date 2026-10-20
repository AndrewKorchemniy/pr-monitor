use super::*;

#[test]
fn hex_parse_formats() {
    assert_eq!("#16181C".parse(), Ok(Rgba8::rgb(0x16, 0x18, 0x1C)));
    assert_eq!("#16181c".parse(), Ok(Rgba8::rgb(0x16, 0x18, 0x1C)));
    assert_eq!(
        " #00000080 ".parse(),
        Ok(Rgba8 {
            r: 0,
            g: 0,
            b: 0,
            a: 0x80
        })
    );
    for bad in ["16181C", "#16181", "#16181C0", "#GG181C", "", "#", "red"] {
        assert!(bad.parse::<Rgba8>().is_err(), "{bad:?}");
    }
}

#[test]
fn hex_display_roundtrip() {
    for text in ["#16181C", "#00000080", "#FFFFFF"] {
        let color: Rgba8 = text.parse().unwrap();
        assert_eq!(color.to_string(), text);
    }
}

#[test]
fn serde_uses_hex_strings() {
    #[derive(serde::Deserialize, serde::Serialize)]
    struct Holder {
        color: Rgba8,
    }
    let holder: Holder = toml::from_str(r##"color = "#86C8EA""##).unwrap();
    assert_eq!(holder.color, Rgba8::rgb(0x86, 0xC8, 0xEA));
    assert_eq!(
        toml::to_string(&holder).unwrap().trim(),
        r##"color = "#86C8EA""##
    );
    assert!(toml::from_str::<Holder>(r#"color = "blue""#).is_err());
}
