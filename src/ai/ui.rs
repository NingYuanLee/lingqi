//! 灵动岛里的 AI 抽屉：输入、流式输出。
//!
//! 这里是唯一允许出现 egui 控件的地方（文本输入需要真实键盘）。
//! 岛体的其它界面是「自绘 + 临时 Ui」，这里沿用同一套路：
//! 用 `egui::Ui::new` 在面板区域里开一块临时画布放控件。

use crate::app::App;
use crate::paint::Card;

/// 抽屉展开时的面板高度。
pub const AI_HEIGHT: f32 = 236.0;

/// 画 AI 抽屉。
pub fn draw(app: &mut App, c: &Card) {
    let ctx = c.ui.ctx().clone();
    let rect = c.ui.max_rect();
    let mut ui = eframe::egui::Ui::new(
        ctx.clone(),
        eframe::egui::Id::new(("lingqi-ai", c.live)),
        eframe::egui::UiBuilder::new()
            .layer_id(c.ui.layer_id())
            .max_rect(rect)
            .style(c.ui.style().clone()),
    );

    // 已有回答：流式过程中会越来越长。
    if !app.ai.answer.is_empty() {
        let answer = app.ai.answer.clone();
        ui.add(eframe::egui::Label::new(eframe::egui::RichText::new(answer).size(12.0)).wrap());
    }
    if let Some(error) = app.ai.error.clone() {
        ui.add(eframe::egui::Label::new(
            eframe::egui::RichText::new(error)
                .size(11.5)
                .color(eframe::egui::Color32::from_rgb(226, 96, 96)),
        ));
    }
    if app.ai.streaming {
        ui.spinner();
        ctx.request_repaint_after(std::time::Duration::from_millis(80));
    }

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(2.0);

    let response = ui.add(
        eframe::egui::TextEdit::singleline(&mut app.ai.draft)
            .hint_text("问点什么，或说「记一下 …」")
            .desired_width(ui.available_width()),
    );
    let submitted = response.lost_focus() && ui.input(|input| input.key_pressed(eframe::egui::Key::Enter));
    if submitted {
        submit(app);
    }
}

/// 发起一次对话。
fn submit(app: &mut App) {
    let text = app.ai.draft.trim().to_string();
    if text.is_empty() || app.ai.streaming {
        return;
    }
    if !app.settings.ai_enabled {
        app.ai.error = Some("AI 助手还没打开，先到设置里打开并填好 API Key。".into());
        return;
    }
    let key = match crate::ai::credentials::load(crate::ai::credentials::SLOT_DEEPSEEK) {
        Ok(key) => key,
        Err(error) => {
            app.ai.error = Some(error);
            return;
        }
    };

    let context = crate::ai::context::collect(&app.ai_sources());
    let mut messages = vec![crate::ai::provider::Message::system(
        crate::ai::prompt::system_prompt(&context, &crate::ai::intents::Intent::ALL),
    )];
    for turn in &app.ai.history {
        messages.push(crate::ai::provider::Message { role: turn.role, content: turn.content.clone() });
    }
    messages.push(crate::ai::provider::Message::user(text.clone()));

    let model = if app.settings.ai_model.trim().is_empty() {
        crate::ai::provider::DEFAULT_MODEL.to_string()
    } else {
        app.settings.ai_model.trim().to_string()
    };
    let request = crate::ai::provider::ChatRequest {
        model,
        messages,
        json_mode: false,
        temperature: 0.6,
        max_tokens: 1024,
    };

    app.ai.answer.clear();
    app.ai.error = None;
    app.ai.draft.clear();
    app.ai.push_turn(crate::ai::provider::Role::User, text);
    app.ai.streaming = true;
    app.ai.stream = crate::ai::provider::spawn_stream(request, key);
}
