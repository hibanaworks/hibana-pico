use crate::protocol::{
    AdmittedReplyInput, BoundedText, Generation, Hash, MAX_BODY_BYTES, ProtocolError, ReplyId,
    UntrustedReplyObject, hash_pair,
};

pub struct LlmBoundary;

impl LlmBoundary {
    pub fn propose_text(bytes: &[u8]) -> Result<BoundedText<MAX_BODY_BYTES>, ProtocolError> {
        BoundedText::new(bytes)
    }

    pub fn admitted_reply_text(
        admitted: &AdmittedReplyInput,
        reply: &UntrustedReplyObject,
    ) -> Result<BoundedText<MAX_BODY_BYTES>, ProtocolError> {
        if admitted.reply_id != reply.reply_id()
            || admitted.object_id != reply.object_id()
            || admitted.body_hash != reply.body_hash()
        {
            return Err(ProtocolError::ReplyNotAdmitted);
        }
        BoundedText::new(reply.body().as_bytes())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodexAccountFingerprint(pub Hash);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodexAuthMode {
    ChatGptManaged,
    ApiKeyFallback(Hash),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodexTurnRequest {
    pub reply_id: ReplyId,
    pub generation: Generation,
    pub input_hash: Hash,
    pub prompt_hash: Hash,
    pub account: CodexAccountFingerprint,
    input: BoundedText<MAX_BODY_BYTES>,
}

impl CodexTurnRequest {
    pub const fn input(&self) -> &BoundedText<MAX_BODY_BYTES> {
        &self.input
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodexTurnResponse {
    pub reply_id: ReplyId,
    pub generation: Generation,
    pub input_hash: Hash,
    pub proposal: BoundedText<MAX_BODY_BYTES>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodexAppServerError {
    ReplyNotAdmitted,
    BackendUnavailable,
    MismatchedResponse,
    ProposalRejected,
}

pub trait CodexAppServer {
    fn turn(&mut self, request: CodexTurnRequest)
    -> Result<CodexTurnResponse, CodexAppServerError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodexAppServerBoundary {
    account: CodexAccountFingerprint,
    auth: CodexAuthMode,
}

impl CodexAppServerBoundary {
    pub const fn new(account: CodexAccountFingerprint, auth: CodexAuthMode) -> Self {
        Self { account, auth }
    }

    pub const fn auth(&self) -> CodexAuthMode {
        self.auth
    }

    pub fn reply_turn_request(
        &self,
        admitted: &AdmittedReplyInput,
        reply: &UntrustedReplyObject,
    ) -> Result<CodexTurnRequest, CodexAppServerError> {
        let input = match LlmBoundary::admitted_reply_text(admitted, reply) {
            Ok(input) => input,
            Err(error) => {
                return Err(match error {
                    ProtocolError::ReplyNotAdmitted => CodexAppServerError::ReplyNotAdmitted,
                    ProtocolError::TextTooLong | ProtocolError::ReplyApprovalMismatch => {
                        CodexAppServerError::ProposalRejected
                    }
                });
            }
        };
        Ok(CodexTurnRequest {
            reply_id: admitted.reply_id,
            generation: admitted.generation,
            input_hash: input.hash(),
            prompt_hash: hash_pair(input.hash(), Hash(0xC0DE_0001)),
            account: self.account,
            input,
        })
    }

    pub fn propose_reply_draft(
        &self,
        admitted: &AdmittedReplyInput,
        reply: &UntrustedReplyObject,
        app_server: &mut impl CodexAppServer,
    ) -> Result<BoundedText<MAX_BODY_BYTES>, CodexAppServerError> {
        let request = self.reply_turn_request(admitted, reply)?;
        let response = app_server.turn(request)?;
        if response.reply_id != request.reply_id
            || response.generation != request.generation
            || response.input_hash != request.input_hash
        {
            return Err(CodexAppServerError::MismatchedResponse);
        }
        Ok(response.proposal)
    }
}
