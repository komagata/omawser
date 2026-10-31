//! Window-local tab ordering. Native widgets and page state stay owned by the caller.
pub struct Tabs<T> {
    items: Vec<T>,
    active: Option<usize>,
    closed: Vec<String>,
}
impl<T> Default for Tabs<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            active: None,
            closed: Vec::new(),
        }
    }
}
impl<T> Tabs<T> {
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn items(&self) -> &[T] {
        &self.items
    }
    pub fn active_index(&self) -> Option<usize> {
        self.active
    }
    pub fn active(&self) -> Option<&T> {
        self.active.and_then(|i| self.items.get(i))
    }
    pub fn push(&mut self, item: T, foreground: bool) {
        self.items.push(item);
        if foreground || self.active.is_none() {
            self.active = Some(self.items.len() - 1);
        }
    }
    pub fn select(&mut self, index: usize) {
        if index < self.items.len() {
            self.active = Some(index);
        }
    }
    pub fn cycle(&mut self, delta: isize) {
        if let Some(index) = self.active {
            self.active =
                Some((index as isize + delta).rem_euclid(self.items.len() as isize) as usize);
        }
    }
    pub fn close(&mut self, index: usize, url: &str) -> Option<T> {
        if index >= self.items.len() {
            return None;
        }
        let item = self.items.remove(index);
        self.active = match self.active {
            _ if self.items.is_empty() => None,
            Some(active) if active > index => Some(active - 1),
            Some(active) if active == index => Some(index.min(self.items.len() - 1)),
            active => active,
        };
        if crate::navigation_allowed(url) {
            self.closed.push(url.into());
            if self.closed.len() > 20 {
                self.closed.remove(0);
            }
        }
        Some(item)
    }
    pub fn reopen(&mut self) -> Option<String> {
        self.closed.pop()
    }
}
