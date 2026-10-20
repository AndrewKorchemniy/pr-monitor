//! About: version, build, renderer, privacy, links and third-party licenses.

use eframe::egui::{self, RichText};
use prm_domain::app::DocLink;
use prm_service::Command;

use crate::{actions::UiAction, copy, frame::FrameCx, screens::settings::SettingsEnv, widgets};

pub(crate) fn page(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, env: &SettingsEnv<'_>) {
    let theme = cx.theme.clone();
    let space = theme.tokens.metrics.space.lg;
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
    ui.add_space(space);
    ui.add(egui::Label::new(copy::PRIVACY).wrap());
    ui.add_space(space);
    ui.horizontal(|ui| {
        for (label, url) in [
            (copy::SOURCE_CODE, prm_domain::app::REPO_URL),
            (copy::REPORT_PROBLEM, DocLink::Troubleshooting.url()),
        ] {
            if ui.link(label).clicked()
                && let Ok(url) = url::Url::parse(url)
            {
                cx.push(UiAction::Send(Command::OpenUrl(url)));
            }
        }
    });
    ui.add_space(space);
    egui::CollapsingHeader::new(copy::LICENSES).show(ui, |ui| {
        egui::ScrollArea::vertical()
            .max_height(320.0)
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
    });
}
