//! Repositories: one module per migration-owning ticket (`<ticket>_*.rs`), discovered by `build.rs` so adding one edits no
//! shared list. Repositories take a `&Connection` (a `WriteTx::conn()` inside a transition, or a read connection) and never
//! open transactions themselves.

include!(concat!(env!("OUT_DIR"), "/repositories.rs"));
