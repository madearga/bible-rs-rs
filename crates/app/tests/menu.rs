// SPDX-License-Identifier: MIT
// Copyright (c) 2026 madearga

//! The menu bar parity tests.
//!
//! These tests inspect the menu description and the bridge, never AppKit
//! itself, so they run on a build machine with no display and while a window is
//! open. What that leaves untested is the native drawing of the menu and the
//! click that AppKit delivers, which the launch check in the report covers by
//! hand.

#![cfg(target_os = "macos")]

use std::collections::HashSet;
use std::sync::mpsc;

use bible_app::macos_menu::{
    Item, KEYBOARD_ONLY, accelerator_for, action_for_id, code_for, drain_actions, menu_id,
    menu_key, menu_spec, route,
};
use bible_app::shortcuts::{Action, Combo, Key, SHORTCUTS, combo_for};
use muda::accelerator::{Accelerator, Code, Modifiers};

/// Every command item in the tree, with the name of the menu holding it.
fn command_items() -> Vec<(String, Action, String, Option<Combo>)> {
    let mut found = Vec::new();
    for menu in menu_spec() {
        for item in menu.items {
            if let Item::Action { action, label, key } = item {
                found.push((menu.title.clone(), action, label, key));
            }
        }
    }
    found
}

#[test]
fn the_menu_bar_has_the_expected_menus() {
    let titles: Vec<String> = menu_spec().into_iter().map(|menu| menu.title).collect();
    assert_eq!(
        titles,
        [
            "Bible",
            "File",
            "Go",
            "View",
            "Translations",
            "Window",
            "Help"
        ]
    );
}

#[test]
fn there_is_no_edit_menu() {
    for menu in menu_spec() {
        assert_ne!(menu.title, "Edit", "an Edit menu would swallow command-C");
    }
}

#[test]
fn the_menu_bar_has_the_expected_items() {
    let tree: Vec<(String, Vec<String>)> = menu_spec()
        .into_iter()
        .map(|menu| {
            let items = menu
                .items
                .iter()
                .map(|item| match item {
                    Item::Action { label, .. } => label.clone(),
                    Item::Predefined(kind) => format!("{kind:?}"),
                    Item::Separator => "-".to_owned(),
                })
                .collect();
            (menu.title, items)
        })
        .collect();

    let expected: Vec<(&str, Vec<&str>)> = vec![
        ("Bible", vec!["About", "Settings", "-", "Quit"]),
        ("File", vec!["Export Markdown", "Export JSON"]),
        (
            "Go",
            vec![
                "Jump",
                "Search",
                "Book",
                "-",
                "Back",
                "Forward",
                "-",
                "Previous Chapter",
                "Next Chapter",
            ],
        ),
        (
            "View",
            vec![
                "Toggle Sidebar",
                "Cycle Theme",
                "Words of Christ in Red",
                "Larger Text",
                "Smaller Text",
                "Default Text Size",
                "-",
                "Verse Numbers",
                "Section Headings",
            ],
        ),
        (
            "Translations",
            vec!["KJV", "WEB", "BSB", "ASV", "YLT", "DARBY", "WEBSTER"],
        ),
        ("Window", vec!["Minimize", "Zoom", "-", "BringAllToFront"]),
        ("Help", vec!["Keyboard Shortcuts"]),
    ];

    let expected: Vec<(String, Vec<String>)> = expected
        .into_iter()
        .map(|(title, items)| {
            (
                title.to_owned(),
                items.into_iter().map(str::to_owned).collect(),
            )
        })
        .collect();
    assert_eq!(tree, expected);
}

#[test]
fn every_menu_command_is_unique() {
    let mut seen = HashSet::new();
    for (_, action, label, _) in command_items() {
        assert!(seen.insert(action), "{label} is in the menu twice");
    }
}

#[test]
fn every_menu_command_is_performed_by_the_keyboard_handler() {
    let table: Vec<Action> = SHORTCUTS.iter().map(|shortcut| shortcut.action).collect();
    for (menu, action, label, _) in command_items() {
        if action == Action::Preferences
            || action == Action::BookPicker
            || action == Action::ToggleVerseNumbers
            || action == Action::ToggleHeadings
            || action == Action::ShowShortcuts
        {
            // These have no key combination, so the keyboard handler cannot
            // reach them and the menu bar is their only menu entry. The book
            // and chapter picker also opens from the toolbar button.
            assert!(
                combo_for(action).is_none(),
                "{label} gained a shortcut without a menu accelerator"
            );
            continue;
        }
        assert!(
            table.contains(&action),
            "{menu} item {label} has no keyboard action"
        );
    }
}

#[test]
fn every_shortcut_has_exactly_one_menu_item_or_is_declared_keyboard_only() {
    let menu_actions: Vec<Action> = command_items()
        .into_iter()
        .map(|(_, action, _, _)| action)
        .collect();
    let mut missing = Vec::new();
    for shortcut in SHORTCUTS {
        if !menu_actions.contains(&shortcut.action) && !KEYBOARD_ONLY.contains(&shortcut.action) {
            missing.push(shortcut.action);
        }
    }
    assert!(
        missing.is_empty(),
        "these shortcuts have no menu item and are not declared keyboard only: {missing:?}"
    );

    let mut stale = Vec::new();
    for action in KEYBOARD_ONLY {
        if menu_actions.contains(action) {
            stale.push(*action);
        }
        if !SHORTCUTS.iter().any(|shortcut| shortcut.action == *action) {
            stale.push(*action);
        }
    }
    assert!(
        stale.is_empty(),
        "these declared keyboard only actions are menu items or have no shortcut: {stale:?}"
    );

    // The declared list has to be exactly the difference, so a new shortcut and
    // a new menu item cannot both slip in without the other.
    // An action may have more than one combination, so the difference is a set.
    // Bookmark is one action with two keys and must still appear once.
    let mut difference: Vec<Action> = Vec::new();
    for shortcut in SHORTCUTS {
        if !menu_actions.contains(&shortcut.action) && !difference.contains(&shortcut.action) {
            difference.push(shortcut.action);
        }
    }
    for action in &difference {
        assert!(
            KEYBOARD_ONLY.contains(action),
            "{action:?} is in the shortcut table but not the menu and not declared"
        );
    }
    let declared: HashSet<Action> = KEYBOARD_ONLY.iter().copied().collect();
    assert_eq!(
        declared.len(),
        KEYBOARD_ONLY.len(),
        "the declared list repeats an action"
    );
    assert_eq!(
        difference.len(),
        KEYBOARD_ONLY.len(),
        "the menu bar and the declared keyboard only list disagree: {difference:?}"
    );
}

#[test]
fn a_plain_key_is_not_a_menu_accelerator() {
    // AppKit gives an unmodified item the whole keyboard, so these keep their
    // key through the keyboard handler and show no accelerator in the menu.
    for action in [
        Action::ExportMarkdown,
        Action::PreviousChapter,
        Action::NextChapter,
    ] {
        assert!(
            combo_for(action).is_some_and(|combo| !combo.command && !combo.option),
            "{action:?} is expected to have a plain table combination"
        );
        assert_eq!(
            menu_key(action),
            None,
            "{action:?} would capture a plain key"
        );
    }
    for (menu, _action, label, key) in command_items() {
        if let Some(combo) = key {
            assert!(
                combo.command || combo.option,
                "{menu} item {label} claims the unmodified key {combo:?}"
            );
        }
    }
}

#[test]
fn every_menu_accelerator_matches_the_shortcut_table() {
    for (menu, action, label, key) in command_items() {
        let table = menu_key(action);
        assert_eq!(
            key, table,
            "{menu} item {label} shows {key:?}, the table says {table:?}"
        );
        if let Some(combo) = key {
            let accelerator = accelerator_for(combo).expect("a table combination is expressible");
            let mut modifiers = Modifiers::empty();
            if combo.command {
                modifiers |= Modifiers::META;
            }
            if combo.shift {
                modifiers |= Modifiers::SHIFT;
            }
            if combo.option {
                modifiers |= Modifiers::ALT;
            }
            assert_eq!(
                accelerator.modifiers(),
                modifiers,
                "{label} has wrong modifiers"
            );
            assert_eq!(
                accelerator.key(),
                code_for(combo.key).expect("every table key has a native code"),
                "{label} has the wrong key"
            );
        }
    }
}

#[test]
fn the_accelerators_use_the_native_codes() {
    let expected = [
        (Combo::command(Key::Char('k')), Modifiers::META, Code::KeyK),
        (Combo::command(Key::Char('f')), Modifiers::META, Code::KeyF),
        (Combo::command(Key::Char('b')), Modifiers::META, Code::KeyB),
        (
            Combo::command(Key::Char('1')),
            Modifiers::META,
            Code::Digit1,
        ),
        (
            Combo::command(Key::Char('7')),
            Modifiers::META,
            Code::Digit7,
        ),
        (
            Combo::command(Key::Char('[')),
            Modifiers::META,
            Code::BracketLeft,
        ),
        (
            Combo::command(Key::Char(']')),
            Modifiers::META,
            Code::BracketRight,
        ),
        (Combo::command(Key::Char('+')), Modifiers::META, Code::Equal),
        (Combo::command(Key::Char('-')), Modifiers::META, Code::Minus),
        (
            Combo::command(Key::Char('0')),
            Modifiers::META,
            Code::Digit0,
        ),
        (Combo::option(Key::Left), Modifiers::ALT, Code::ArrowLeft),
        (Combo::option(Key::Right), Modifiers::ALT, Code::ArrowRight),
        (
            Combo::command_shift(Key::Char('c')),
            Modifiers::META | Modifiers::SHIFT,
            Code::KeyC,
        ),
    ];
    for (combo, modifiers, code) in expected {
        assert_eq!(
            accelerator_for(combo),
            Some(Accelerator::new(modifiers, code)),
            "{combo:?} does not become {modifiers:?} {code:?}"
        );
    }
    assert_eq!(accelerator_for(Combo::plain(Key::Char('?'))), None);
}

#[test]
fn every_menu_identifier_maps_back_to_its_action() {
    let spec = menu_spec();
    for (_, action, label, _) in command_items() {
        assert_eq!(
            action_for_id(&menu_id(action), &spec),
            Some(action),
            "{label} does not route back to its action"
        );
    }
    assert_eq!(action_for_id("action.not-a-command", &spec), None);
    assert_eq!(action_for_id("", &spec), None);
}

#[test]
fn the_bridge_queues_the_action_for_an_identifier() {
    let spec = menu_spec();
    let (sender, receiver) = mpsc::channel();
    route(&menu_id(Action::Search), &spec, &sender);
    route(&menu_id(Action::Translation(2)), &spec, &sender);
    route("action.unknown", &spec, &sender);
    assert_eq!(receiver.try_recv(), Ok(Action::Search));
    assert_eq!(receiver.try_recv(), Ok(Action::Translation(2)));
    assert!(receiver.try_recv().is_err());
}

#[test]
fn the_translations_menu_lists_every_bundled_translation() {
    let translations = menu_spec()
        .into_iter()
        .find(|menu| menu.title == "Translations")
        .expect("the menu bar has a Translations menu");
    assert_eq!(
        translations.items.len(),
        bible_app::BUNDLED_TRANSLATIONS.len()
    );
    for (index, item) in translations.items.iter().enumerate() {
        let Item::Action { action, label, key } = item else {
            panic!("the Translations menu holds a command at position {index}");
        };
        assert_eq!(*action, Action::Translation(index as u8));
        assert_eq!(
            *label,
            bible_app::BUNDLED_TRANSLATIONS[index].id().to_uppercase()
        );
        assert_eq!(
            *key,
            combo_for(Action::Translation(index as u8)),
            "{label} does not carry its table combination"
        );
    }
}

#[test]
fn the_queue_is_empty_before_a_menu_event() {
    assert!(drain_actions().is_empty());
}
