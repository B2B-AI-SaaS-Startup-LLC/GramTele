//! Actual UsernamesInfo value model, including its surprising editable-index rules.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Username {
    pub username: String,
    pub active: bool,
    pub editable: bool,
}
pub type Usernames = Vec<Username>;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsernamesInfo {
    usernames: Vec<String>,
    editable: Option<usize>,
}
impl UsernamesInfo {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_username(&mut self, username: &str) {
        if self.usernames.is_empty() || self.editable.is_none_or(|i| i >= self.usernames.len()) {
            if username.is_empty() {
                self.editable = None;
            } else {
                self.usernames.push(username.into());
                self.editable = Some(0);
            }
        } else if self.usernames[self.editable.unwrap()] != username {
            let i = self.editable.unwrap();
            if username.is_empty() {
                self.usernames.remove(i);
                self.editable = None;
            } else {
                self.usernames[i] = username.into();
            }
        }
    }
    pub fn set_usernames(&mut self, values: &[Username]) {
        let mut editable = "";
        self.usernames = values
            .iter()
            .filter_map(|v| {
                if v.editable {
                    editable = &v.username;
                }
                v.active.then(|| v.username.clone())
            })
            .collect();
        if editable.is_empty() {
            self.editable = None;
        } else if let Some(i) = self.usernames.iter().position(|s| s == editable) {
            self.editable = Some(i);
        }
        // Original retains the PREVIOUS index when nonempty editable is inactive.
    }
    pub fn username(&self) -> &str {
        self.usernames.first().map_or("", String::as_str)
    }
    pub fn editable_username(&self) -> &str {
        self.editable.map_or("", |i| self.usernames[i].as_str())
    }
    pub fn usernames(&self) -> &[String] {
        &self.usernames
    }
    pub fn is_editable(&self, username: &str) -> bool {
        self.editable
            .and_then(|i| self.usernames.get(i))
            .is_some_and(|s| s == username)
    }
}
