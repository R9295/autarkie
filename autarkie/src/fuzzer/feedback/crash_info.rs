#![allow(warnings)]
use std::{borrow::Cow, marker::PhantomData, path::PathBuf};

use libafl::{
    corpus::Testcase,
    executors::ExitKind,
    feedbacks::{Feedback, StateInitializer},
    observers::StdErrObserver,
    state::{HasCorpus, HasCurrentTestcase},
    Error, HasMetadata,
};
use libafl_bolts::{
    tuples::{Handle, MatchNameRef},
    Named,
};

use crate::{Input, Node};

pub struct CrashInfoFeedback<I> {
    enabled: bool,
    stderr_handle: Handle<StdErrObserver>,
    solutions_dir: PathBuf,
    last_exit_kind: ExitKind,
    phantom: PhantomData<I>,
}

impl<I> CrashInfoFeedback<I> {
    pub fn new(
        enabled: bool,
        stderr_handle: Handle<StdErrObserver>,
        solutions_dir: PathBuf,
    ) -> Self {
        Self {
            enabled,
            stderr_handle,
            solutions_dir,
            last_exit_kind: ExitKind::Ok,
            phantom: PhantomData,
        }
    }
}

fn format_crash_report(exit_kind: &ExitKind, name: &str, captured: Option<&[u8]>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"autarkie crash report\n");
    out.extend_from_slice(format!("input: {name}\n").as_bytes());
    out.extend_from_slice(format!("exit_kind: {exit_kind:?}\n").as_bytes());
    out.extend_from_slice(b"\n");
    match captured {
        Some(bytes) if !bytes.is_empty() => out.extend_from_slice(bytes),
        _ => out.extend_from_slice(b"____no crash output captured\n"),
    }
    out
}

impl<I, EM, OT, S> Feedback<EM, I, OT, S> for CrashInfoFeedback<I>
where
    I: Node + Input,
    OT: MatchNameRef,
    S: HasCurrentTestcase<I> + HasCorpus<I> + HasMetadata,
{
    fn is_interesting(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _input: &I,
        _observers: &OT,
        exit_kind: &ExitKind,
    ) -> Result<bool, Error> {
        if self.enabled {
            self.last_exit_kind = *exit_kind;
        }
        Ok(false)
    }

    fn append_metadata(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        observers: &OT,
        testcase: &mut Testcase<I>,
    ) -> Result<(), Error> {
        if !self.enabled {
            return Ok(());
        }
        let name = if let Some(existing) = testcase.filename().clone() {
            existing
        } else {
            let generated = testcase
                .input()
                .as_ref()
                .expect("____crashInfoNoInput")
                .generate_name(None);
            *testcase.filename_mut() = Some(generated.clone());
            generated
        };
        let captured = observers
            .get(&self.stderr_handle)
            .and_then(|observer| observer.output.clone());
        let report = format_crash_report(&self.last_exit_kind, &name, captured.as_deref());
        let path = self.solutions_dir.join(format!("{name}.txt"));
        if let Err(e) = std::fs::write(&path, report) {
            eprintln!("____crashInfoWriteFailed {}: {e}", path.display());
        }
        Ok(())
    }
}

impl<I, S> StateInitializer<S> for CrashInfoFeedback<I> {}

impl<I> Named for CrashInfoFeedback<I> {
    fn name(&self) -> &Cow<'static, str> {
        &Cow::Borrowed("CrashInfoFeedback")
    }
}

#[cfg(test)]
mod tests {
    use super::format_crash_report;
    use libafl::executors::ExitKind;

    #[test]
    fn report_includes_header_and_body() {
        let report = format_crash_report(
            &ExitKind::Crash,
            "deadbeef",
            Some(b"==1==ERROR: AddressSanitizer: heap-buffer-overflow"),
        );
        let text = String::from_utf8(report).unwrap();
        assert!(text.contains("autarkie crash report"));
        assert!(text.contains("input: deadbeef"));
        assert!(text.contains("exit_kind: Crash"));
        assert!(text.contains("AddressSanitizer"));
    }

    #[test]
    fn report_uses_placeholder_when_empty() {
        let report = format_crash_report(&ExitKind::Crash, "abc123", None);
        let text = String::from_utf8(report).unwrap();
        assert!(text.contains("input: abc123"));
        assert!(text.contains("no crash output captured"));
    }
}
