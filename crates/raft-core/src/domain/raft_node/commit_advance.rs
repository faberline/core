use super::RaftNode;
use crate::domain::role::Role;

impl RaftNode {
    /// Leader: advance `commit_index` to the highest index replicated to a
    /// majority of **voters** (both incoming and outgoing sets if joint)
    /// whose entry is from the current term.
    pub(super) fn maybe_commit(&mut self) {
        if self.role != Role::Leader {
            return;
        }
        let last = self.last_index();
        let mut new_commit = self.commit_index;
        let incoming_maj = self.conf_state.membership.voters.len() / 2 + 1;
        let outgoing_maj = self
            .conf_state
            .outgoing
            .as_ref()
            .map(|out| out.len() / 2 + 1);

        for n in (self.commit_index + 1)..=last {
            if self.term_at(n) != self.current_term {
                continue;
            }
            let mut incoming_count = 0usize;
            for v in &self.conf_state.membership.voters {
                let m = if *v == self.id {
                    last
                } else {
                    *self.match_index.get(v).unwrap_or(&0)
                };
                if m >= n {
                    incoming_count += 1;
                }
            }
            let incoming_ok = incoming_count >= incoming_maj;

            let outgoing_ok = match &self.conf_state.outgoing {
                Some(outgoing) => {
                    let mut outgoing_count = 0usize;
                    for v in outgoing {
                        let m = if *v == self.id {
                            last
                        } else {
                            *self.match_index.get(v).unwrap_or(&0)
                        };
                        if m >= n {
                            outgoing_count += 1;
                        }
                    }
                    outgoing_count >= outgoing_maj.unwrap()
                }
                None => true,
            };

            if incoming_ok && outgoing_ok {
                new_commit = n;
            }
        }
        self.commit_index = new_commit;
    }
}
