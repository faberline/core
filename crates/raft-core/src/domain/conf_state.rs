use serde::{Deserialize, Serialize};

use super::ids::NodeId;
use super::membership::Membership;

/// Monotonically sequenced cluster membership.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfState {
    pub membership: Membership,
    #[serde(default)]
    pub outgoing: Option<Vec<NodeId>>,
    pub generation: u64,
}

impl ConfState {
    pub fn encode(&self) -> Vec<u8> {
        let outgoing_len = self.outgoing.as_ref().map(|o| o.len()).unwrap_or(0);
        let mut buf = Vec::with_capacity(
            24 + (self.membership.voters().len() + self.membership.learners().len() + outgoing_len)
                * 8
                + 8,
        );
        buf.extend_from_slice(&self.generation.to_le_bytes());
        buf.extend_from_slice(&(self.membership.voters().len() as u64).to_le_bytes());
        for &v in self.membership.voters() {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(&(self.membership.learners().len() as u64).to_le_bytes());
        for &l in self.membership.learners() {
            buf.extend_from_slice(&l.to_le_bytes());
        }
        match &self.outgoing {
            Some(outgoing) => {
                buf.extend_from_slice(&(outgoing.len() as u64).to_le_bytes());
                for &v in outgoing {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
            }
            None => {
                buf.extend_from_slice(&0u64.to_le_bytes());
            }
        }
        buf
    }

    pub fn decode_with_len(bytes: &[u8]) -> Option<(ConfState, usize)> {
        if bytes.len() < 24 {
            return None;
        }
        let mut offset = 0;
        let generation = u64::from_le_bytes(bytes[offset..offset + 8].try_into().ok()?);
        offset += 8;
        let voters_len = u64::from_le_bytes(bytes[offset..offset + 8].try_into().ok()?) as usize;
        offset += 8;
        if voters_len > (bytes.len() - offset - 8) / 8 {
            return None;
        }
        let mut voters = Vec::with_capacity(voters_len);
        for _ in 0..voters_len {
            voters.push(u64::from_le_bytes(
                bytes[offset..offset + 8].try_into().ok()?,
            ));
            offset += 8;
        }
        let learners_len = u64::from_le_bytes(bytes[offset..offset + 8].try_into().ok()?) as usize;
        offset += 8;
        if learners_len > (bytes.len() - offset) / 8 {
            return None;
        }
        let mut learners = Vec::with_capacity(learners_len);
        for _ in 0..learners_len {
            learners.push(u64::from_le_bytes(
                bytes[offset..offset + 8].try_into().ok()?,
            ));
            offset += 8;
        }
        let outgoing = if offset == bytes.len() {
            None
        } else {
            if bytes.len() - offset < 8 {
                return None;
            }
            let outgoing_len =
                u64::from_le_bytes(bytes[offset..offset + 8].try_into().ok()?) as usize;
            offset += 8;
            if outgoing_len == 0 {
                None
            } else {
                if outgoing_len > (bytes.len() - offset) / 8 {
                    return None;
                }
                let mut outgoing_voters = Vec::with_capacity(outgoing_len);
                for _ in 0..outgoing_len {
                    outgoing_voters.push(u64::from_le_bytes(
                        bytes[offset..offset + 8].try_into().ok()?,
                    ));
                    offset += 8;
                }
                Some(outgoing_voters)
            }
        };
        Some((
            ConfState {
                membership: Membership::new(voters, learners),
                outgoing,
                generation,
            },
            offset,
        ))
    }

    pub fn decode(bytes: &[u8]) -> Option<ConfState> {
        Self::decode_with_len(bytes).map(|(conf, _)| conf)
    }
}
