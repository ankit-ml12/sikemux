//! Find in a chat: ⌘F opens a bar over the transcript, which marks every place the text
//! is found and brings the one the reader is on into view.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{px, AppContext, Context, Entity, Focusable, Subscription, Window};
use gpui_transcript::Query;

use crate::ui::FindBar;
use crate::workbench::Workbench;

/// The find bar of a chat, while it is open.
pub struct Finding {
    input: Entity<InputState>,
    match_case: bool,
    whole_word: bool,
    _typed: Subscription,
}

impl Workbench {
    /// Opens the find bar of `agent`'s chat, or puts the reader back in it.
    pub fn open_find(&mut self, agent: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chat) = self.chats.get_mut(agent) else {
            return;
        };
        if chat.finding.is_none() {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Find in chat"));
            let typed = {
                let agent = agent.to_owned();
                cx.subscribe(&input, move |bench, _, event: &InputEvent, cx| {
                    if let InputEvent::Change = event {
                        bench.search(&agent, cx);
                    }
                })
            };
            chat.finding = Some(Finding {
                input,
                match_case: false,
                whole_word: false,
                _typed: typed,
            });
        }
        if let Some(finding) = &chat.finding {
            finding
                .input
                .update(cx, |input, cx| input.select_all(window, cx));
            finding.input.read(cx).focus_handle(cx).focus(window, cx);
        }
        self.parts.redraw(&self.parts.stage, cx);
    }

    /// Looks for what the bar holds, as its options say.
    fn search(&mut self, agent: &str, cx: &mut Context<Self>) {
        let Some(chat) = self.chats.get(agent) else {
            return;
        };
        let query = chat.finding.as_ref().map(|finding| Query {
            text: finding.input.read(cx).value().to_string(),
            match_case: finding.match_case,
            whole_word: finding.whole_word,
        });
        let query = query.filter(|query| !query.text.is_empty());
        chat.unfold_for_find(query.is_some(), cx);
        chat.transcript
            .update(cx, |transcript, cx| transcript.find(query, cx));
        self.parts.redraw(&self.parts.stage, cx);
    }

    fn flip_find(&mut self, agent: &str, flip: fn(&mut Finding), cx: &mut Context<Self>) {
        if let Some(finding) = self
            .chats
            .get_mut(agent)
            .and_then(|chat| chat.finding.as_mut())
        {
            flip(finding);
            self.search(agent, cx);
        }
    }

    fn step_find(&mut self, agent: &str, forward: bool, cx: &mut Context<Self>) {
        if let Some(chat) = self.chats.get(agent) {
            chat.transcript
                .update(cx, |transcript, cx| transcript.step(forward, cx));
        }
    }

    /// Closes the bar, takes the marks away, and puts the reader back in the draft.
    pub fn close_find(&mut self, agent: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chat) = self.chats.get_mut(agent) else {
            return;
        };
        chat.finding = None;
        chat.unfold_for_find(false, cx);
        chat.transcript
            .update(cx, |transcript, cx| transcript.find(None, cx));
        if let Some(composing) = &chat.composer {
            composing.draft.read(cx).focus_handle(cx).focus(window, cx);
        }
        self.parts.redraw(&self.parts.stage, cx);
    }
}

/// The find bar over `agent`'s transcript, while it is open.
pub fn bar(
    agent: &str,
    finding: &Finding,
    found: (usize, usize),
    cx: &mut Context<Workbench>,
) -> FindBar {
    let label = match found {
        (current, total) if total > 0 => format!("{}/{total}", current + 1),
        _ if finding.input.read(cx).value().is_empty() => "0/0".to_owned(),
        _ => "No results".to_owned(),
    };
    let this = cx.weak_entity();
    let on = |act: fn(&mut Workbench, &str, &mut Window, &mut Context<Workbench>)| {
        let (this, agent) = (this.clone(), agent.to_owned());
        move |window: &mut Window, cx: &mut gpui_kit::App| {
            this.update(cx, |bench, cx| act(bench, &agent, window, cx))
                .ok();
        }
    };
    let step = {
        let (this, agent) = (this.clone(), agent.to_owned());
        move |forward: bool, _: &mut Window, cx: &mut gpui_kit::App| {
            this.update(cx, |bench, cx| bench.step_find(&agent, forward, cx))
                .ok();
        }
    };
    FindBar::new(&finding.input, label)
        .below(px(46.))
        .match_case(
            finding.match_case,
            on(|bench, agent, _, cx| {
                bench.flip_find(agent, |finding| finding.match_case ^= true, cx)
            }),
        )
        .whole_word(
            finding.whole_word,
            on(|bench, agent, _, cx| {
                bench.flip_find(agent, |finding| finding.whole_word ^= true, cx)
            }),
        )
        .on_step(step)
        .on_close(on(|bench, agent, window, cx| {
            bench.close_find(agent, window, cx)
        }))
}
