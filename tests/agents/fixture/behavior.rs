// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/11 18:02:55.287476
//+ Revised: 	2026/11/12 10:31:43.718039
// -----------------------------------------------------------------------------

use super::*;
impl Fixture {
    pub(super) fn prompt(&mut self, frame: Value) -> anyhow::Result<()> {
        self.prompt = Some(frame["id"].clone());
        if self.mode == "prompt-error" {
            io::stderr().write_all(b"ACP_STDERR_SECRET_MUST_NOT_BE_LOGGED\n")?;
            return emit(
                json!({"jsonrpc":"2.0","id":self.prompt.take(),"error":{"code":-32603,"message":"ACP_ERROR_SECRET_MUST_NOT_BE_LOGGED"}}),
            );
        }
        if self.mode == "crash" {
            std::process::exit(17);
        }
        if self.mode == "oversized" {
            io::stdout().write_all(&vec![b'x'; 4 * 1_024 * 1_024 + 2])?;
            io::stdout().write_all(b"\n")?;
            io::stdout().flush()?;
            return Ok(());
        }
        if frame["params"]["prompt"][0]["text"] == "plan first" {
            // The agent switches its own mode, as adapters do when asked to plan.
            self.mode_choice = "plan".into();
            update(json!({"sessionUpdate":"current_mode_update","currentModeId":"plan"}))?;
            return self.complete("end_turn");
        }
        if frame["params"]["prompt"][0]["text"] == "go deep" {
            self.model = "fixture-deep".into();
            update(
                json!({"sessionUpdate":"config_option_update","configOptions":self.config_options()}),
            )?;
            return self.complete("end_turn");
        }
        if frame["params"]["prompt"][0]["text"] == "hold" {
            return update(
                json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Waiting for cancellation."}}),
            );
        }
        update(
            json!({"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"Reviewing the requested change."}}),
        )?;
        update(
            json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"I can update src/main.rs."}}),
        )?;
        if self.mode == "unsupported" {
            emit(
                json!({"jsonrpc":"2.0","id":"fs-request","method":"fs/read_text_file","params":{"sessionId":"fixture-session","path":"outside"}}),
            )?;
            return emit(
                json!({"jsonrpc":"2.0","id":"terminal-request","method":"terminal/create","params":{"sessionId":"fixture-session","command":"must-not-run"}}),
            );
        }
        if self.mode == "normal" || self.mode == "child" {
            return self.complete("end_turn");
        }
        let tool = self.tool();
        update(tool.clone())?;
        let mut request = json!({"jsonrpc":"2.0","id":"permission-1","method":"session/request_permission",
            "params":{"sessionId":"fixture-session","toolCall":{"toolCallId":"edit-1","title":"Update src/main.rs","kind":"edit"},
            "options":[{"optionId":"allow-once","name":"Allow once","kind":"allow_once"},
            {"optionId":"reject-once","name":"Reject","kind":"reject_once"}]}});
        if self.mode == "bad-permission" {
            request["params"]["options"][1]["optionId"] = json!("allow-once");
        }
        emit(request)
    }

    pub(super) fn permission_response(&mut self, frame: Value) -> anyhow::Result<()> {
        if frame["result"]["outcome"]["outcome"] == "cancelled" {
            return self.complete("cancelled");
        }
        let allowed = frame["result"]["outcome"]["optionId"] == "allow-once";
        if allowed {
            fs::create_dir_all(self.cwd.join("src"))?;
            fs::write(self.cwd.join("src/main.rs"), new_text())?;
        }
        update(
            json!({"sessionUpdate":"tool_call_update","toolCallId":"edit-1","status":if allowed {"completed"} else {"failed"}}),
        )?;
        update(
            json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":if allowed {"Updated src/main.rs."} else {"Change rejected."}}}),
        )?;
        self.complete("end_turn")
    }
}
