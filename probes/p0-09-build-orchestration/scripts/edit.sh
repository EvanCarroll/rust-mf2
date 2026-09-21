#!/usr/bin/env bash
# P0.9 incremental-rebuild scenarios: one edit each.
#   text     en: change the text of `activity-available` (no manifest change)
#   pl       pl: change the text of `activity-available` (translation only)
#   addvar   en: add `{$extra}` to `actions-joined-busy` (call sites must now fail)
#   addsite  second crate: use `p09.added-message` (not yet in the locales → must fail)
#   addmsg   en: add `p09.added-message = Freshly added for {$who}.` (now it builds)
#   touch    en: touch common.mf2 without changing it (mtime only)
#   revert   restore locales/ from gen/ and the second crate (extra.rs, lib.rs)
set -euo pipefail
cd "$(dirname "$0")/.."
L=crates/i18n/locales
case "${1:?scenario}" in
    text)   sed -i 's/^activity-available = Result than page still screenshots\.$/activity-available = Result than page still screenshots EDITED-P09./' $L/en/common.mf2 ;;
    pl)     sed -i 's/^activity-available = gonąmo nie ną wałoszy czę powy wieszy\.$/activity-available = PL-EDITED-P09 gonąmo./' $L/pl/common.mf2 ;;
    addvar) sed -i 's/^actions-joined-busy = Is {\$title} a configuration\.$/actions-joined-busy = Is {$title} a configuration {$extra}./' $L/en/common.mf2 ;;
    addsite) cat > crates/second/src/extra.rs <<'RS'
//! Extra `/second` lines (edited by scripts/edit.sh addsite).

use p09_i18n::tr;

pub(crate) fn lines(who: &'static str) -> Vec<(String, u32)> {
    let t = tr!("p09.added-message", who = who);
    vec![(t.to_string(), t.id().0)]
}
RS
    ;;
    addmsg) printf '\n[p09]\n# Added by the P0.9 incremental test.\n@param $who - A name.\nadded-message = Freshly added for {$who}.\n' >> $L/en/common.mf2 ;;
    touch)  touch $L/en/common.mf2 ;;
    revert) rm -rf $L && cp -r gen/locales $L && cp crates/second/extra.rs.orig crates/second/src/extra.rs && cp crates/second/lib.rs.orig crates/second/src/lib.rs ;;
    *) echo "unknown scenario $1" >&2; exit 2 ;;
esac
echo "edit: $1"
