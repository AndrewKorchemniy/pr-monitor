use super::*;

const LOCAL_GPU: RenderFacts = RenderFacts {
    renderer: RendererKind::Glow,
    software_adapter: false,
    remote_session: false,
    os_animations: None,
};

#[test]
fn automatic_is_full_fidelity_on_a_local_gpu() {
    let policy = RenderPolicy::resolve(Tristate::Auto, Tristate::Auto, LOCAL_GPU);
    assert_eq!(
        policy,
        RenderPolicy {
            low_power: false,
            animations: true
        }
    );
}

#[test]
fn automatic_goes_low_power_over_remote_desktop_or_software_rendering() {
    for facts in [
        RenderFacts {
            remote_session: true,
            ..LOCAL_GPU
        },
        RenderFacts {
            software_adapter: true,
            ..LOCAL_GPU
        },
    ] {
        let policy = RenderPolicy::resolve(Tristate::Auto, Tristate::Auto, facts);
        assert_eq!(
            policy,
            RenderPolicy {
                low_power: true,
                animations: false
            }
        );
    }
}

#[test]
fn the_os_animation_setting_and_explicit_choices_win() {
    let os_off = RenderFacts {
        os_animations: Some(false),
        ..LOCAL_GPU
    };
    assert!(!RenderPolicy::resolve(Tristate::Auto, Tristate::Auto, os_off).animations);
    assert!(RenderPolicy::resolve(Tristate::Auto, Tristate::On, os_off).animations);
    let remote = RenderFacts {
        remote_session: true,
        ..LOCAL_GPU
    };
    assert!(!RenderPolicy::resolve(Tristate::Off, Tristate::Auto, remote).low_power);
}
