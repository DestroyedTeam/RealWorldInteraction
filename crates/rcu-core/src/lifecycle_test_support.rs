use super::*;
pub(super) fn window(n: u32) -> WindowEvidence {
    WindowEvidence {
        key: WindowIdentity {
            window: format!("hwnd:{n:x}"),
            pid: 10,
            process_created: 100,
            native_epoch: format!("epoch-{n}"),
        },
        application: ApplicationIdentity {
            executable: r"C:\fixture\app.exe".into(),
            sha256: "a".repeat(64),
        },
        title: "existing".into(),
        foreground: false,
        candidate_facts: Some(WindowCandidateFacts {
            visible: true,
            cloaked: Some(false),
            owned: false,
            owner_chain_complete: true,
        }),
    }
}
