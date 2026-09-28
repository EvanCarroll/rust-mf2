//! A library that renders differently on the server, through `mf2`'s macros.

::mf2::__if_ssr! {
    /// Only in a server build.
    pub fn report() -> String {
        format!("server widgets; mf2 compiled with [{}]", ::mf2::compiled_features())
    }
}
::mf2::__if_not_ssr! {
    /// Everywhere else.
    pub fn report() -> String {
        format!("client widgets; mf2 compiled with [{}]", ::mf2::compiled_features())
    }
}
