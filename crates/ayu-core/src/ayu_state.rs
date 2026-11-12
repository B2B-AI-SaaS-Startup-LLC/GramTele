use std::collections::{HashMap, HashSet};
/// Процессное состояние; владелец находится в главном потоке.
#[derive(Default)]
pub struct AyuState {
    hidden_messages: HashMap<u64, HashSet<i32>>,
    disable_ghost_on_story_close: Option<u64>,
}
impl AyuState {
    pub fn hide(&mut self, peer_id: u64, message_id: i32) {
        self.hidden_messages
            .entry(peer_id)
            .or_default()
            .insert(message_id);
    }
    pub fn is_hidden(&self, peer_id: u64, message_id: i32) -> bool {
        self.hidden_messages
            .get(&peer_id)
            .is_some_and(|ids| ids.contains(&message_id))
    }
    pub fn set_disable_ghost_mode_on_story_close(&mut self, session: Option<u64>) {
        self.disable_ghost_on_story_close = session;
    }
    /// Возвращает токен Session; Core находит по нему аккаунт и вызывает setGhostModeEnabled(false).
    pub fn disable_ghost_mode_on_story_close(&mut self, session: Option<u64>) -> Option<u64> {
        if self.disable_ghost_on_story_close != session {
            return None;
        }
        self.disable_ghost_on_story_close = None;
        session
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hidden_and_one_shot_story_owner() {
        let mut s = AyuState::default();
        s.hide(1, 4);
        s.hide(1, 4);
        assert!(s.is_hidden(1, 4));
        assert!(!s.is_hidden(2, 4));
        s.set_disable_ghost_mode_on_story_close(Some(9));
        assert_eq!(s.disable_ghost_mode_on_story_close(Some(8)), None);
        assert_eq!(s.disable_ghost_mode_on_story_close(Some(9)), Some(9));
        assert_eq!(s.disable_ghost_mode_on_story_close(Some(9)), None);
    }
}
