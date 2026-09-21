struct Errs(u8);
impl mf2dt::ErrSink for Errs {
    fn push(&mut self, e: mf2dt::Error) {
        self.0 |= match e {
            mf2dt::Error::BadOperand => 1,
            mf2dt::Error::BadOption => 2,
            mf2dt::Error::UnsupportedOperation => 4,
        };
    }
}
/// Output = text, then `\u{1}` and one char carrying the error bits.
fn finish(mut s: String, e: &Errs) -> String {
    s.push('\u{1}');
    s.push(char::from(b'0' + e.0));
    s
}
