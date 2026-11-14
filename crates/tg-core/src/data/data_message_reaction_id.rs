//! ReactionId value/TL contract only; search/hashtag/entity helpers remain package 09.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReactionId {
    Emoji(String),
    Custom(u64),
}
impl Default for ReactionId {
    fn default() -> Self {
        Self::Emoji(String::new())
    }
}
impl ReactionId {
    pub fn paid_tag() -> char {
        '*'
    }
    pub fn paid_id() -> Self {
        Self::Emoji("*".into())
    }
    pub fn empty(&self) -> bool {
        matches!(self,Self::Emoji(s) if s.is_empty())
    }
    pub fn paid(&self) -> bool {
        matches!(self,Self::Emoji(s) if s=="*")
    }
    pub fn emoji(&self) -> &str {
        match self {
            Self::Emoji(s) if s != "*" => s,
            _ => "",
        }
    }
    pub fn custom(&self) -> u64 {
        match self {
            Self::Custom(id) => *id,
            _ => 0,
        }
    }
}
impl Ord for ReactionId {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        match (self, o) {
            (Self::Emoji(a), Self::Emoji(b)) => a.encode_utf16().cmp(b.encode_utf16()),
            (Self::Emoji(_), _) => std::cmp::Ordering::Less,
            (_, Self::Emoji(_)) => std::cmp::Ordering::Greater,
            (Self::Custom(a), Self::Custom(b)) => a.cmp(b),
        }
    }
}
impl PartialOrd for ReactionId {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
pub fn reaction_from_mtp(value: &tg_scheme::Reaction) -> ReactionId {
    use tg_scheme::Reaction::*;
    match value {
        ReactionEmpty(_) => ReactionId::default(),
        ReactionPaid(_) => ReactionId::paid_id(),
        ReactionCustomEmoji(d) => ReactionId::Custom(d.document_id),
        ReactionEmoji(d) => ReactionId::Emoji(String::from_utf8_lossy(&d.emoticon).into()),
    }
}
pub fn reaction_to_mtp(id: &ReactionId) -> tg_scheme::Reaction {
    if id.empty() {
        tg_scheme::mtp_reaction_empty()
    } else if id.paid() {
        tg_scheme::mtp_reaction_paid()
    } else if id.custom() != 0 {
        tg_scheme::mtp_reaction_custom_emoji(id.custom())
    } else {
        tg_scheme::mtp_reaction_emoji(id.emoji().as_bytes().into())
    }
}
