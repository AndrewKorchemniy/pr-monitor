use super::*;

fn chrome(caption: [u8; 3]) -> ChromeColors {
    ChromeColors {
        caption,
        text: [200, 200, 200],
        border: [60, 60, 60],
    }
}

#[test]
fn dark_captions_ask_for_dark_title_bars() {
    let cases = [
        ([0x16, 0x18, 0x1C], true),
        ([0x70, 0x70, 0x70], true),
        ([0x80, 0x80, 0x80], false),
        ([0xEE, 0xF1, 0xF4], false),
    ];
    for (caption, dark) in cases {
        assert_eq!(title_bar(chrome(caption)).dark, dark, "{caption:?}");
    }
}

#[test]
fn every_color_is_passed_through() {
    let colors = title_bar(chrome([1, 2, 3]));
    assert_eq!(colors.caption, Some(Rgb { r: 1, g: 2, b: 3 }));
    assert_eq!(
        colors.text,
        Some(Rgb {
            r: 200,
            g: 200,
            b: 200
        })
    );
    assert_eq!(
        colors.border,
        Some(Rgb {
            r: 60,
            g: 60,
            b: 60
        })
    );
}
