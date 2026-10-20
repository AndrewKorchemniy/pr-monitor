//! About: version, build, renderer, privacy, links and third-party licenses.

use eframe::egui::{self, RichText};
use prm_domain::app::DocLink;
use prm_service::Command;

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    screens::settings::SettingsEnv,
    widgets::{self, card::section},
};

pub(crate) fn page(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, env: &SettingsEnv<'_>) {
    let theme = cx.theme.clone();
    section(ui, &theme, copy::APP_NAME, None, |ui| {
        ui.label(
            RichText::new(copy::version_line(env.build.version))
                .font(theme.fonts.display.clone())
                .color(theme.colors.text_primary),
        );
        widgets::muted(ui, &theme, copy::ABOUT_LINE);
        let build = format!("{} · {}", env.build.git_sha, env.build.target);
        ui.label(
            RichText::new(build)
                .font(theme.fonts.mono.clone())
                .color(theme.colors.text_secondary),
        );
        widgets::muted(ui, &theme, env.renderer_line);
    });
    section(ui, &theme, copy::SECTION_PRIVACY, None, |ui| {
        ui.add(egui::Label::new(copy::PRIVACY).wrap());
    });
    section(ui, &theme, copy::SECTION_LINKS, None, |ui| links(ui, cx));
    section(ui, &theme, copy::LICENSES, None, |ui| {
        licenses(ui, &theme, env)
    });
}

fn links(ui: &mut egui::Ui, cx: &mut FrameCx<'_>) {
    let mut opened = None;
    ui.horizontal(|ui| {
        for (label, url) in [
            (copy::SOURCE_CODE, prm_domain::app::REPO_URL),
            (copy::REPORT_PROBLEM, DocLink::Troubleshooting.url()),
        ] {
            if ui.link(label).clicked() {
                opened = url::Url::parse(url).ok();
            }
        }
    });
    if let Some(url) = opened {
        cx.push(UiAction::Send(Command::OpenUrl(url)));
    }
}

fn licenses(ui: &mut egui::Ui, theme: &crate::theme::UiTheme, env: &SettingsEnv<'_>) {
    egui::ScrollArea::vertical()
        .max_height(240.0)
        .show(ui, |ui| {
            for text in [
                env.licenses,
                crate::theme::fonts::OFL_NEXT,
                crate::theme::fonts::OFL_MONO,
            ] {
                ui.label(
                    RichText::new(text)
                        .font(theme.fonts.mono.clone())
                        .color(theme.colors.text_secondary),
                );
                ui.separator();
            }
        });
}
