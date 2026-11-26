//! Source control: a Git panel built on the `git` command line.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariant, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    notification::Notification,
    v_flex,
};
use gpui_kit::base::Disableable as _;
use gpui_kit::{prelude::FluentBuilder as _, *};

use crate::icons;

#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub path: String,
    /// Index (staged) status letter, ' ' when unchanged.
    pub staged: char,
    /// Worktree status letter, ' ' when unchanged.
    pub worktree: char,
}

impl Change {
    pub fn is_staged(&self) -> bool {
        self.staged != ' ' && self.staged != '?'
    }
    pub fn is_unstaged(&self) -> bool {
        self.worktree != ' ' || self.staged == '?'
    }
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub changes: Vec<Change>,
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("PATH", crate::util::login_path())
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("git not found: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { "git command failed".into() } else { err })
    }
}

pub fn is_repo(root: &Path) -> bool {
    root.join(".git").exists()
}

/// Parse `git status --porcelain=v1 -b`.
pub fn parse_status(text: &str) -> Status {
    let mut status = Status::default();
    for line in text.lines() {
        if let Some(head) = line.strip_prefix("## ") {
            let (name, rest) = head.split_once("...").map_or((head, ""), |(a, b)| (a, b));
            let name = name.split(' ').next().unwrap_or(name);
            status.branch = Some(name.trim_start_matches("No commits yet on ").to_string());
            if let (Some(a), Some(b)) = (rest.find('['), rest.find(']')) {
                for part in rest[a + 1..b].split(", ") {
                    if let Some(n) = part.strip_prefix("ahead ") {
                        status.ahead = n.parse().unwrap_or(0);
                    } else if let Some(n) = part.strip_prefix("behind ") {
                        status.behind = n.parse().unwrap_or(0);
                    }
                }
            }
            continue;
        }
        if line.len() < 4 {
            continue;
        }
        let mut chars = line.chars();
        let x = chars.next().unwrap_or(' ');
        let y = chars.next().unwrap_or(' ');
        let mut path = line[3..].to_string();
        if let Some((_, to)) = path.split_once(" -> ") {
            path = to.to_string();
        }
        let path = path.trim_matches('"').to_string();
        status.changes.push(Change { path, staged: x, worktree: y });
    }
    status
}

pub fn status(root: &Path) -> Result<Status, String> {
    git(root, &["status", "--porcelain=v1", "-b", "-uall"]).map(|t| parse_status(&t))
}

/// A unified diff for one file (staged or working tree; untracked files
/// diff against nothing).
pub fn diff(root: &Path, path: &str, staged: bool, untracked: bool) -> Result<String, String> {
    if untracked {
        // `git diff --no-index` exits 1 when files differ, which is the point.
        let out = Command::new("git")
            .args(["diff", "--no-color", "--no-index", "--", "/dev/null", path])
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let mut args = vec!["diff", "--no-color"];
    if staged {
        args.push("--cached");
    }
    args.push("--");
    args.push(path);
    git(root, &args)
}

#[derive(Clone)]
pub enum GitEvent {
    OpenDiff { path: String, staged: bool, untracked: bool },
    Changed(usize),
}

impl EventEmitter<GitEvent> for GitPanel {}

pub struct GitPanel {
    root: Option<PathBuf>,
    pub status: Status,
    message: Entity<InputState>,
    busy: Option<&'static str>,
    _task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl GitPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let message = cx.new(|cx| InputState::new(window, cx).placeholder("Message (⌘↩ to commit)"));
        let sub = cx.subscribe_in(&message, window, |this, _, event, window, cx| {
            if let InputEvent::PressEnter { secondary: true, .. } = event {
                this.commit(window, cx);
            }
        });
        Self {
            root: None,
            status: Status::default(),
            message,
            busy: None,
            _task: None,
            _subscriptions: vec![sub],
        }
    }

    pub fn set_root(&mut self, root: Option<PathBuf>, cx: &mut Context<Self>) {
        self.root = root;
        self.status = Status::default();
        self.refresh(cx);
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone().filter(|r| is_repo(r)) else {
            self.status = Status::default();
            cx.notify();
            return;
        };
        let task = cx.background_spawn(async move { status(&root) });
        self._task = Some(cx.spawn(async move |this, cx| {
            if let Ok(status) = task.await {
                let _ = this.update(cx, |this, cx| {
                    this.status = status;
                    cx.emit(GitEvent::Changed(this.status.changes.len()));
                    cx.notify();
                });
            }
        }));
    }

    /// Run a git command in the background, then refresh.
    fn run(&mut self, label: &'static str, args: Vec<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else { return };
        self.busy = Some(label);
        cx.notify();
        let task = cx.background_spawn(async move {
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            git(&root, &refs)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = None;
                if let Err(err) = result {
                    window.push_notification(Notification::error(err).title(format!("git {label}")), cx);
                }
                this.refresh(cx);
            });
        })
        .detach();
    }

    fn stage(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.run("add", vec!["add".into(), "--".into(), path.into()], window, cx);
    }

    fn unstage(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.run("restore", vec!["restore".into(), "--staged".into(), "--".into(), path.into()], window, cx);
    }

    fn stage_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.run("add", vec!["add".into(), "-A".into()], window, cx);
    }

    fn discard(&mut self, change: Change, window: &mut Window, cx: &mut Context<Self>) {
        let panel = cx.entity();
        let untracked = change.staged == '?';
        let name = change.path.clone();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let panel = panel.clone();
            let change = change.clone();
            dialog
                .title(format!("Discard changes to “{name}”?"))
                .description(if untracked {
                    "This untracked file will be deleted. This can't be undone."
                } else {
                    "Your uncommitted changes to this file will be lost. This can't be undone."
                })
                .confirm()
                .ok_text("Discard")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    let args = if untracked {
                        vec!["clean".into(), "-f".into(), "--".into(), change.path.clone()]
                    } else {
                        vec!["restore".into(), "--".into(), change.path.clone()]
                    };
                    panel.update(cx, |p, cx| p.run("discard", args, window, cx));
                    true
                })
        });
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let message = self.message.read(cx).value().trim().to_string();
        if message.is_empty() {
            window.push_notification("Write a commit message first", cx);
            return;
        }
        let nothing_staged = !self.status.changes.iter().any(Change::is_staged);
        let mut args = vec!["commit".to_string(), "-m".into(), message];
        if nothing_staged {
            // Like VS Code's smart commit: commit everything when nothing is staged.
            args.insert(1, "-a".into());
            if self.status.changes.iter().all(|c| c.staged == '?') {
                window.push_notification("Stage untracked files before committing", cx);
                return;
            }
        }
        self.message.update(cx, |s, cx| s.set_value("", window, cx));
        self.run("commit", args, window, cx);
    }

    fn render_change(&self, change: &Change, staged: bool, ix: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let letter = if staged { change.staged } else if change.staged == '?' { 'U' } else { change.worktree };
        let color = match letter {
            'M' => theme.warning,
            'A' | 'U' | '?' => theme.success,
            'D' => theme.danger,
            'R' | 'C' => theme.info,
            _ => theme.muted_foreground,
        };
        let path = Path::new(&change.path);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| change.path.clone());
        let dir = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let group = SharedString::from(format!("chg-{staged}-{ix}"));
        let c1 = change.clone();
        let c2 = change.clone();
        let c3 = change.clone();
        h_flex()
            .id(SharedString::from(format!("chg-{staged}-{}", change.path)))
            .group(group.clone())
            .h(px(26.))
            .mx_1p5()
            .px_2()
            .gap_1p5()
            .rounded(px(7.))
            .text_sm()
            .cursor_pointer()
            .hover(|d| d.bg(theme.list_hover))
            .child(img(icons::file_icon(path)).size(px(15.)).flex_shrink_0())
            .child(div().flex_shrink_0().child(name))
            .child(div().flex_1().min_w_0().truncate().text_xs().text_color(theme.muted_foreground).child(dir))
            .child(
                h_flex()
                    .invisible()
                    .group_hover(group, |s| s.visible())
                    .when(!staged, |d| {
                        d.child(
                            Button::new(SharedString::from(format!("discard-{}", change.path)))
                                .ghost()
                                .xsmall()
                                .icon(Lucide::Undo2)
                                .tooltip("Discard Changes")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.discard(c1.clone(), window, cx)
                                })),
                        )
                    })
                    .child(
                        Button::new(SharedString::from(format!("stage-{staged}-{}", change.path)))
                            .ghost()
                            .xsmall()
                            .icon(if staged { IconName::Minus } else { IconName::Plus })
                            .tooltip(if staged { "Unstage" } else { "Stage" })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                if staged {
                                    this.unstage(&c2.path, window, cx)
                                } else {
                                    this.stage(&c2.path, window, cx)
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .w(px(14.))
                    .flex_shrink_0()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color)
                    .child(letter.to_string()),
            )
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(GitEvent::OpenDiff {
                    path: c3.path.clone(),
                    staged,
                    untracked: c3.staged == '?',
                })
            }))
    }
}

impl Render for GitPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Some(root) = self.root.clone() else {
            return div().p_4().text_sm().text_color(theme.muted_foreground).child("Open a folder to use source control.").into_any_element();
        };
        if !is_repo(&root) {
            return v_flex()
                .p_4()
                .gap_3()
                .child(div().text_sm().text_color(theme.muted_foreground).child("This folder isn't a Git repository."))
                .child(
                    Button::new("git-init")
                        .small()
                        .primary()
                        .label("Initialize Repository")
                        .on_click(cx.listener(|this, _, window, cx| this.run("init", vec!["init".into()], window, cx))),
                )
                .into_any_element();
        }
        let staged: Vec<Change> = self.status.changes.iter().filter(|c| c.is_staged()).cloned().collect();
        let unstaged: Vec<Change> = self.status.changes.iter().filter(|c| c.is_unstaged()).cloned().collect();
        let busy = self.busy;
        let ahead = self.status.ahead;
        let behind = self.status.behind;
        let section = |title: String| {
            h_flex()
                .h(px(26.))
                .px_3()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.muted_foreground)
                .child(title)
        };

        v_flex()
            .size_full()
            .child(
                v_flex()
                    .px_2()
                    .pb_2()
                    .gap_1p5()
                    .child(Input::new(&self.message).small())
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("git-commit")
                                    .small()
                                    .primary()
                                    .flex_1()
                                    .icon(IconName::Check)
                                    .label(if staged.is_empty() && !unstaged.is_empty() { "Commit All" } else { "Commit" })
                                    .loading(busy == Some("commit"))
                                    .disabled(self.status.changes.is_empty())
                                    .on_click(cx.listener(|this, _, window, cx| this.commit(window, cx))),
                            )
                            .child(
                                Button::new("git-pull")
                                    .small()
                                    .outline()
                                    .icon(IconName::ArrowDown)
                                    .when(behind > 0, |b| b.label(behind.to_string()))
                                    .loading(busy == Some("pull"))
                                    .tooltip("Pull")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.run("pull", vec!["pull".into(), "--ff-only".into()], window, cx)
                                    })),
                            )
                            .child(
                                Button::new("git-push")
                                    .small()
                                    .outline()
                                    .icon(IconName::ArrowUp)
                                    .when(ahead > 0, |b| b.label(ahead.to_string()))
                                    .loading(busy == Some("push"))
                                    .tooltip("Push")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.run("push", vec!["push".into()], window, cx)
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .id("git-changes")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pb_3()
                    .when(self.status.changes.is_empty(), |d| {
                        d.child(div().px_3().py_2().text_sm().text_color(theme.muted_foreground).child("No changes"))
                    })
                    .when(!staged.is_empty(), |d| {
                        d.child(section(format!("Staged · {}", staged.len())))
                            .children(staged.iter().enumerate().map(|(i, c)| self.render_change(c, true, i, cx).into_any_element()).collect::<Vec<_>>())
                    })
                    .when(!unstaged.is_empty(), |d| {
                        d.child(
                            section(format!("Changes · {}", unstaged.len())).justify_between().child(
                                Button::new("git-stage-all")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Plus)
                                    .tooltip("Stage All")
                                    .on_click(cx.listener(|this, _, window, cx| this.stage_all(window, cx))),
                            ),
                        )
                        .children(unstaged.iter().enumerate().map(|(i, c)| self.render_change(c, false, i, cx).into_any_element()).collect::<Vec<_>>())
                    }),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::parse_status;

    #[core::prelude::v1::test]
    fn parses_porcelain_status() {
        let s = parse_status(
            "## main...origin/main [ahead 2, behind 1]\n M src/a.rs\nA  new.rs\n?? notes.txt\nR  old.rs -> moved.rs\n",
        );
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!((s.ahead, s.behind), (2, 1));
        assert_eq!(s.changes.len(), 4);
        assert!(s.changes[0].is_unstaged() && !s.changes[0].is_staged());
        assert!(s.changes[1].is_staged());
        assert!(s.changes[2].is_unstaged() && !s.changes[2].is_staged());
        assert_eq!(s.changes[3].path, "moved.rs");
    }
}
