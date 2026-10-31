//! Keybinding validation regressions promoted from roadmap item 02.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_dispatch_core::testing::key;
use tui_dispatch_core::{
    format_key_for_display, parse_key_string, BindingContext, ConflictPolicy,
    DefaultBindingContext, Keybindings,
};

#[test]
fn invalid_keys_and_duplicate_assignments_are_reported() {
    let mut bindings = Keybindings::<DefaultBindingContext>::new();
    bindings.add_global("save", vec!["ctrl-s".into()]);
    bindings.add_global("search", vec!["ctrl-s".into()]);
    bindings.add_global("broken", vec!["definitely-not-a-key".into()]);
    let errors = bindings.validate().unwrap_err();
    assert_eq!(
        errors.invalid_keys().collect::<Vec<_>>(),
        vec!["definitely-not-a-key"]
    );
    let conflicts = errors.conflicts().collect::<Vec<_>>();
    assert_eq!(conflicts.len(), 1);
    let mut commands = conflicts[0].commands().collect::<Vec<_>>();
    commands.sort();
    assert_eq!(commands, ["save", "search"]);
}

#[test]
fn replacement_policy_uses_registration_order() {
    let mut bindings = Keybindings::<DefaultBindingContext>::new();
    bindings
        .try_add_global("save", ["ctrl-s"], ConflictPolicy::Reject)
        .unwrap();
    assert!(bindings
        .try_add_global("search", ["ctrl-s"], ConflictPolicy::Reject)
        .is_err());
    assert_eq!(
        bindings.get_command_ref(&key("ctrl-s"), DefaultBindingContext),
        Some("save")
    );
    bindings
        .try_add_global("search", ["ctrl-s"], ConflictPolicy::Replace)
        .unwrap();
    assert_eq!(
        bindings.get_command_ref(&key("ctrl-s"), DefaultBindingContext),
        Some("search")
    );
    bindings.validate().unwrap();
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Context {
    Editor,
    Search,
}

impl BindingContext for Context {
    fn name(&self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Search => "search",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::all()
            .iter()
            .copied()
            .find(|context| context.name() == name)
    }

    fn all() -> &'static [Self] {
        &[Self::Editor, Self::Search]
    }
}

#[test]
fn validation_groups_aliases_and_reports_locations_in_stable_order() {
    let mut bindings = Keybindings::<Context>::new();
    bindings.add_global("z_save", vec![" CONTROL + S ".into(), "ctrl-s".into()]);
    bindings.add_global("a_search", vec!["ctrl+s".into(), "CTRL-S".into()]);
    bindings.add(Context::Search, "bad", vec!["wat+x".into()]);
    bindings.add(Context::Editor, "bad", vec!["".into(), "ctrl+".into()]);
    bindings.add_global("broken", vec!["super+x".into()]);
    let errors = bindings.validate().unwrap_err();
    assert_eq!(
        errors.invalid_keys().collect::<Vec<_>>(),
        ["super+x", "", "ctrl+", "wat+x"]
    );
    let invalid: Vec<_> = errors.invalid_bindings().collect();
    assert_eq!(invalid[0].context, None);
    assert_eq!(invalid[0].command, "broken");
    assert_eq!(invalid[1].context, Some("editor"));
    assert_eq!(invalid[3].context, Some("search"));
    let conflicts: Vec<_> = errors.conflicts().collect();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].context, None);
    assert_eq!(conflicts[0].key, key("ctrl+s"));
    assert_eq!(
        conflicts[0].commands().collect::<Vec<_>>(),
        ["a_search", "z_save"]
    );
    assert!(errors.to_string().contains("[editor] bad: invalid key"));
    assert_eq!(errors, bindings.clone().validate().unwrap_err());
}

#[test]
fn contexts_shadow_globals_without_conflicting_with_each_other() {
    let mut bindings = Keybindings::<Context>::new();
    bindings
        .try_add_global("save", ["ctrl+s"], ConflictPolicy::Reject)
        .unwrap();
    bindings
        .try_add(
            Context::Editor,
            "select",
            ["ctrl-s"],
            ConflictPolicy::Reject,
        )
        .unwrap();
    bindings
        .try_add(
            Context::Search,
            "search",
            ["control+s"],
            ConflictPolicy::Reject,
        )
        .unwrap();
    bindings.validate().unwrap();
    assert_eq!(
        bindings.get_command_ref(&key("ctrl+s"), Context::Editor),
        Some("select")
    );
    assert_eq!(
        bindings.get_command_ref(&key("ctrl+s"), Context::Search),
        Some("search")
    );

    let errors = bindings
        .try_add(Context::Editor, "other", ["ctrl+s"], ConflictPolicy::Reject)
        .unwrap_err();
    assert_eq!(errors.conflicts().next().unwrap().context, Some("editor"));
    bindings
        .try_add(
            Context::Editor,
            "other",
            ["ctrl+s"],
            ConflictPolicy::Replace,
        )
        .unwrap();
    assert_eq!(
        bindings.get_command_ref(&key("ctrl+s"), Context::Editor),
        Some("other")
    );
    assert_eq!(
        bindings.get_command_ref(&key("ctrl+s"), Context::Search),
        Some("search")
    );
    assert_eq!(bindings.global_bindings()["save"], ["ctrl+s"]);

    // Clearing the local command restores global fallback.
    bindings
        .try_add(
            Context::Editor,
            "other",
            Vec::<String>::new(),
            ConflictPolicy::Reject,
        )
        .unwrap();
    assert_eq!(
        bindings.get_command_ref(&key("ctrl+s"), Context::Editor),
        Some("save")
    );
}

#[test]
fn replacement_keeps_unrelated_keys_and_updates_cached_lookup_and_help() {
    let mut bindings = Keybindings::<DefaultBindingContext>::new();
    bindings.add_global(
        "save",
        vec!["ctrl+s".into(), "control-s".into(), "f2".into()],
    );
    assert_eq!(
        bindings.get_command_ref(&key("ctrl+s"), DefaultBindingContext),
        Some("save")
    );
    bindings
        .try_add_global("search", ["ctrl-s"], ConflictPolicy::Replace)
        .unwrap();
    assert_eq!(bindings.global_bindings()["save"], ["f2"]);
    assert_eq!(
        bindings
            .get_first_keybinding("save", DefaultBindingContext)
            .as_deref(),
        Some("f2")
    );
    assert_eq!(
        bindings.get_command_ref(&key("f2"), DefaultBindingContext),
        Some("save")
    );
    assert_eq!(
        bindings.get_command_ref(&key("ctrl+s"), DefaultBindingContext),
        Some("search")
    );
    bindings.validate().unwrap();

    // Re-registering the same command replaces its whole list, including old keys.
    bindings
        .try_add_global("search", ["ctrl+p"], ConflictPolicy::Reject)
        .unwrap();
    assert_eq!(
        bindings.get_command_ref(&key("ctrl+s"), DefaultBindingContext),
        None
    );
    assert_eq!(
        bindings.get_command_ref(&key("ctrl+p"), DefaultBindingContext),
        Some("search")
    );
}

#[test]
fn failed_registration_is_atomic_for_both_policies() {
    for policy in [ConflictPolicy::Reject, ConflictPolicy::Replace] {
        let mut bindings = Keybindings::<Context>::new();
        bindings.add_global("save", vec!["ctrl+s".into()]);
        bindings.add_global("search", vec!["ctrl+p".into()]);
        assert_eq!(
            bindings.get_command_ref(&key("ctrl+s"), Context::Editor),
            Some("save")
        );
        let before = bindings.global_bindings().clone();
        let errors = bindings
            .try_add_global("search", ["ctrl+s", "invalid+q"], policy)
            .unwrap_err();
        assert_eq!(errors.invalid_keys().collect::<Vec<_>>(), ["invalid+q"]);
        assert_eq!(*bindings.global_bindings(), before);
        assert_eq!(
            bindings.get_command_ref(&key("ctrl+s"), Context::Editor),
            Some("save")
        );
        assert_eq!(
            bindings.get_command_ref(&key("ctrl+p"), Context::Editor),
            Some("search")
        );
        assert!(bindings
            .try_add(Context::Search, "bad", ["ctrl+"], policy)
            .is_err());
        assert!(bindings.get_context_bindings(Context::Search).is_none());
    }
}

#[test]
fn unchecked_collisions_are_deterministic_across_order_clone_and_merge() {
    for commands in [["zeta", "alpha"], ["alpha", "zeta"]] {
        let mut bindings = Keybindings::<Context>::new();
        for command in commands {
            bindings.add_global(command, vec!["q".into()]);
            bindings.add(Context::Editor, command, vec!["q".into()]);
        }
        for bindings in [&bindings, &bindings.clone()] {
            assert_eq!(
                bindings.get_command_ref(&key("q"), Context::Search),
                Some("alpha")
            );
            assert_eq!(
                bindings.get_command_ref(&key("q"), Context::Editor),
                Some("alpha")
            );
            assert_eq!(bindings.validate().unwrap_err().conflicts().count(), 2);
        }
        let mut user = Keybindings::new();
        user.add_global("alpha", vec!["x".into()]);
        let merged = Keybindings::merge(bindings, user);
        assert_eq!(
            merged.get_command_ref(&key("q"), Context::Search),
            Some("zeta")
        );
        assert_eq!(
            merged.get_command_ref(&key("x"), Context::Search),
            Some("alpha")
        );
    }
}

#[test]
fn parser_rejects_malformed_input_and_normalizes_supported_aliases() {
    for invalid in [
        "", "ctrl+", "ctrl-", "bogus+x", "+x", "ctrl++x", "ctrl+-x", "f13",
    ] {
        assert_eq!(parse_key_string(invalid), None, "{invalid:?}");
    }
    for (alias, canonical) in [
        (" CTRL - S ", "control+s"),
        ("shift-tab", "backtab"),
        ("ctrl+shift+tab", "ctrl+backtab"),
        ("shift+ctrl-tab", "control-backtab"),
        ("escape", "esc"),
        ("return", "enter"),
        ("ctrl--", "ctrl+-"),
        ("ctrl-+", "ctrl++"),
        ("É", "é"),
    ] {
        assert_eq!(key(alias), key(canonical), "{alias}");
    }
    assert_eq!(key("+").code, KeyCode::Char('+'));
    assert_eq!(key("-").code, KeyCode::Char('-'));
    assert_eq!(format_key_for_display("ctrl-s"), "^S");
    assert_eq!(format_key_for_display("ctrl++"), "^+");
    assert_eq!(format_key_for_display("shift-tab"), "Shift+Tab");
}

#[test]
fn repeated_aliases_on_one_command_are_valid_and_shift_tab_matches_events() {
    let mut bindings = Keybindings::<DefaultBindingContext>::new();
    bindings
        .try_add_global(
            "back",
            ["shift-tab", "backtab", "shift+tab"],
            ConflictPolicy::Reject,
        )
        .unwrap();
    bindings.validate().unwrap();
    let event = KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT);
    assert_eq!(
        bindings.get_command_ref(&event, DefaultBindingContext),
        Some("back")
    );
    let errors = bindings
        .try_add_global("other", ["backtab"], ConflictPolicy::Reject)
        .unwrap_err();
    assert_eq!(errors.conflicts().count(), 1);
}

#[cfg(feature = "serde")]
#[test]
fn deserialized_and_merged_configurations_can_be_validated() {
    let mut defaults = Keybindings::<Context>::new();
    defaults.add_global("save", vec!["ctrl+s".into()]);
    let user = serde_json::from_str(
        r#"{
        "global": {"search": ["ctrl-s"], "bad": ["typo+x"]},
        "editor": {"zeta": ["q"], "alpha": ["q"]}
    }"#,
    )
    .unwrap();
    let merged = Keybindings::merge(defaults, user);
    let errors = merged.validate().unwrap_err();
    assert_eq!(errors.invalid_keys().collect::<Vec<_>>(), ["typo+x"]);
    assert_eq!(errors.conflicts().count(), 2);
    assert_eq!(
        merged.get_command_ref(&key("ctrl+s"), Context::Search),
        Some("save")
    );
    assert_eq!(
        merged.get_command_ref(&key("q"), Context::Editor),
        Some("alpha")
    );
    let roundtrip: Keybindings<Context> =
        serde_json::from_str(&serde_json::to_string(&merged).unwrap()).unwrap();
    assert_eq!(roundtrip.validate().unwrap_err(), errors);
    assert_eq!(
        roundtrip.get_command_ref(&key("q"), Context::Editor),
        Some("alpha")
    );
}
