# mf2-ratatui

Ratatui text from Rust MF2 messages. `text` and `line` format a call site
through [`mf2-native`](https://docs.rs/mf2-native) into owned Ratatui `Text`
and `Line` values, with the message's markup as styles:

```mf2
status = {#ok}Connected{/ok} to {#host}{$host}{/host}.
```

```rust,ignore
let styles = MarkupStyles::new()
    .with("ok", Style::new().fg(Color::Green).bold())
    .with("host", Style::new().underlined());
let line = mf2_ratatui::line(&i18n, &my_i18n::tr!("status", host = host), &styles);
```

The application names the styles; the translation decides where they go.
It depends on `ratatui-core`, whose types `ratatui` re-exports.

See the [native application guide](https://evancarroll.github.io/rust-mf2/native-apps.html).

## License

MIT (`LICENSE`).
