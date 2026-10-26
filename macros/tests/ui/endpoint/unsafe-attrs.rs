#[eva::endpoint]
#[unsafe(no_mangle)]
pub async fn Exported(state: (), args: ()) {}

#[eva::endpoint]
#[unsafe(export_name = "renamed")]
#[unsafe(link_section = ".text.endpoint")]
pub async fn Renamed(state: (), args: ()) {}

// The pre-2024 spelling.
#[eva::endpoint]
#[no_mangle]
pub async fn Bare(state: (), args: ()) {}

fn main() {
    let _ = (Exported, Renamed, Bare);
}
