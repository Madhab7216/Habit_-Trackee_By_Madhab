# Bengali and English localization

## Supported interface languages

The GTK application shell, Today workflow, habit lifecycle dialogs, Goals workspace and milestone dialogs ship with English and Bengali gettext catalogs.

- English uses source-language strings with an explicit English catalog.
- Bengali uses the `bn` catalog.
- The selected language code is saved in the existing SQLite `settings` table under `ui_language`; no schema migration or remote preference service is used.
- The language choice is applied on the next application launch. The selector shows a restart note after the preference is saved.
- User-entered habit names, goal titles and milestone titles are not translated or modified.

## Catalog layout

```text
locale/
  en/LC_MESSAGES/habit-trackers-by-madhab.po
  en/LC_MESSAGES/habit-trackers-by-madhab.mo
  bn/LC_MESSAGES/habit-trackers-by-madhab.po
  bn/LC_MESSAGES/habit-trackers-by-madhab.mo
```

The runtime uses GLib's `dgettext` wrapper with the application domain and binds the catalog directory before the GTK application is constructed. MO files are committed so a normal Rust build does not need `msgfmt` or a network step. The corresponding PO files remain the editable translator source.

## Adding or changing a message

1. Wrap user-visible fixed strings in `i18n::tr("Source English text")`.
2. For interpolated strings, use numbered placeholders and `i18n::format("Target {0}", &[value])`; translators can reorder placeholders.
3. Add the same source key to both PO catalogs and supply the Bengali translation.
4. Regenerate the MO files from the PO catalogs.
5. Run `python3 -m unittest discover -s tests -p 'test_*.py' -v`, then the full Rust format/build/test/Clippy checks.

Do not translate user-entered records, SQL status values, enum names, command names or database keys. Those are data/API values, not UI labels.

## Scope and validation boundary

This increment localizes the existing Today and Goals/Milestones UI present on its base branch. The separate Progress Calendar feature and the companion data-tool CLI are not part of this branch and are not claimed as localized here. A manual Bengali-font and screen-reader review is still required on the target Linux Mint desktop.
