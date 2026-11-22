//! Installed-apps view state: filtering, sorting and update selection.
//! Qt-free; the bridge maps rows of [`InstalledView::rows`] to QML.

pub use kiosque_core::entries::InstalledEntry;

/// Sorting criteria understood by [`InstalledView::sort`].
pub const SORT_CRITERIA: [&str; 3] = ["name", "app_id", "size"];

#[derive(Debug, Clone)]
pub struct InstalledView {
    items: Vec<InstalledEntry>,
    rows: Vec<usize>,
    pub search: String,
    pub updates_only: bool,
    pub show_runtimes: bool,
    criterion: String,
    ascending: bool,
}

impl Default for InstalledView {
    fn default() -> Self {
        Self {
            items: vec![],
            rows: vec![],
            search: String::new(),
            updates_only: false,
            show_runtimes: false,
            criterion: "name".into(),
            ascending: true,
        }
    }
}

impl InstalledView {
    /// Replace the entries, keep the sort order and recompute the rows.
    /// Update selections of entries that are still pending survive.
    pub fn set_items(&mut self, mut items: Vec<InstalledEntry>) {
        for item in &mut items {
            if let Some(old) = self.items.iter().find(|o| o.app_id == item.app_id)
                && old.has_update
                && item.has_update
            {
                item.is_checked_for_update = old.is_checked_for_update;
            }
        }
        self.items = items;
        sort_items_helper(&mut self.items, &self.criterion, self.ascending);
        self.refilter();
    }

    pub fn items(&self) -> &[InstalledEntry] {
        &self.items
    }

    /// Indices into [`items`](Self::items) that pass the filter, in display order.
    pub fn rows(&self) -> &[usize] {
        &self.rows
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The entry shown at `row`.
    pub fn row(&self, row: usize) -> Option<&InstalledEntry> {
        self.rows.get(row).and_then(|&i| self.items.get(i))
    }

    pub fn set_filter(&mut self, search: &str, updates_only: bool, show_runtimes: bool) {
        self.search = search.to_string();
        self.updates_only = updates_only;
        self.show_runtimes = show_runtimes;
        self.refilter();
    }

    pub fn sort(&mut self, criterion: &str, ascending: bool) {
        self.criterion = criterion.to_string();
        self.ascending = ascending;
        sort_items_helper(&mut self.items, &self.criterion, ascending);
        self.refilter();
    }

    fn refilter(&mut self) {
        let filter = self.search.to_lowercase();
        self.rows = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                // Runtimes are hidden unless the user has toggled them on.
                if item.is_runtime && !self.show_runtimes {
                    return false;
                }
                let matches_search = filter.is_empty()
                    || item.name.to_lowercase().contains(&filter)
                    || item.app_id.to_lowercase().contains(&filter);
                matches_search && (!self.updates_only || item.has_update)
            })
            .map(|(i, _)| i)
            .collect();
    }

    /// Flip the update checkbox of `row`. Returns true if the row changed.
    pub fn toggle_checked(&mut self, row: usize) -> bool {
        let Some(&i) = self.rows.get(row) else {
            return false;
        };
        let item = &mut self.items[i];
        if !item.has_update {
            return false;
        }
        item.is_checked_for_update = !item.is_checked_for_update;
        true
    }

    /// Check or uncheck every pending update. Returns the changed rows.
    pub fn set_all_checked(&mut self, checked: bool) -> Vec<usize> {
        let mut changed = vec![];
        for (row, &i) in self.rows.iter().enumerate() {
            let item = &mut self.items[i];
            if item.has_update && item.is_checked_for_update != checked {
                item.is_checked_for_update = checked;
                changed.push(row);
            }
        }
        for item in &mut self.items {
            if item.has_update {
                item.is_checked_for_update = checked;
            }
        }
        changed
    }

    /// `(available, checked)` update counts over all entries.
    pub fn update_counts(&self) -> (usize, usize) {
        let available = self.items.iter().filter(|i| i.has_update).count();
        let checked = self
            .items
            .iter()
            .filter(|i| i.has_update && i.is_checked_for_update)
            .count();
        (available, checked)
    }

    /// Ids of the updates the user selected.
    pub fn selected_updates(&self) -> Vec<String> {
        self.items
            .iter()
            .filter(|i| i.has_update && i.is_checked_for_update)
            .map(|i| i.app_id.clone())
            .collect()
    }
}

/// Human-readable size for an installed size in bytes.
pub use kiosque_core::entries::format_size;

pub fn parse_size_to_bytes(size_str: &str) -> u64 {
    let size_str = size_str.trim().to_lowercase();
    let parts: Vec<&str> = size_str.split_whitespace().collect();
    if parts.is_empty() {
        return 0;
    }
    let num_val: f64 = parts[0].parse().unwrap_or(0.0);
    if parts.len() < 2 {
        return num_val as u64;
    }
    let unit = parts[1];
    if unit.contains("gb") || unit.contains("g") {
        (num_val * 1024.0 * 1024.0 * 1024.0) as u64
    } else if unit.contains("mb") || unit.contains("m") {
        (num_val * 1024.0 * 1024.0) as u64
    } else if unit.contains("kb") || unit.contains("k") {
        (num_val * 1024.0) as u64
    } else {
        num_val as u64
    }
}

pub fn sort_items_helper(items: &mut [InstalledEntry], criterion: &str, ascending: bool) {
    items.sort_by(|a, b| {
        // Runtimes always sort last regardless of other criteria
        if a.is_runtime != b.is_runtime {
            return if a.is_runtime {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            };
        }
        // Items with updates sort before up-to-date items
        if a.has_update != b.has_update {
            return if a.has_update {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }

        match criterion {
            "name" | "" => {
                let res = a.name.to_lowercase().cmp(&b.name.to_lowercase());
                if ascending { res } else { res.reverse() }
            }
            "app_id" => {
                let res = a.app_id.to_lowercase().cmp(&b.app_id.to_lowercase());
                if ascending { res } else { res.reverse() }
            }
            "size" => {
                let size_a = if a.size_bytes > 0 {
                    a.size_bytes
                } else {
                    parse_size_to_bytes(&a.size)
                };
                let size_b = if b.size_bytes > 0 {
                    b.size_bytes
                } else {
                    parse_size_to_bytes(&b.size)
                };
                let res = size_a.cmp(&size_b);
                if ascending { res } else { res.reverse() }
            }
            _ => std::cmp::Ordering::Equal,
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, name: &str, size: &str, update: bool, runtime: bool) -> InstalledEntry {
        InstalledEntry {
            name: name.into(),
            app_id: id.into(),
            size: size.into(),
            has_update: update,
            is_checked_for_update: update,
            is_runtime: runtime,
            ..Default::default()
        }
    }

    fn sample() -> InstalledView {
        let mut v = InstalledView::default();
        v.set_items(vec![
            entry("org.b.Beta", "beta", "2.0 MB", false, false),
            entry("org.a.Alpha", "Alpha", "300 kB", true, false),
            entry("org.kde.Platform", "KDE Platform", "1.2 GB", true, true),
            entry("org.c.Gamma", "Gamma", "10.0 MB", false, false),
        ]);
        v
    }

    fn ids(v: &InstalledView) -> Vec<&str> {
        (0..v.len())
            .map(|r| v.row(r).unwrap().app_id.as_str())
            .collect()
    }

    #[test]
    fn updates_first_runtimes_hidden_and_last() {
        let mut v = sample();
        assert_eq!(ids(&v), ["org.a.Alpha", "org.b.Beta", "org.c.Gamma"]);
        v.set_filter("", false, true);
        assert_eq!(ids(&v).last(), Some(&"org.kde.Platform"));
        v.set_filter("", true, false);
        assert_eq!(ids(&v), ["org.a.Alpha"]);
        v.set_filter("", true, true);
        assert_eq!(ids(&v), ["org.a.Alpha", "org.kde.Platform"]);
        v.set_filter("GAM", false, false);
        assert_eq!(ids(&v), ["org.c.Gamma"]);
    }

    #[test]
    fn sorting_by_size_and_direction() {
        let mut v = sample();
        v.sort("size", false);
        assert_eq!(ids(&v), ["org.a.Alpha", "org.c.Gamma", "org.b.Beta"]);
        v.sort("name", false);
        assert_eq!(ids(&v), ["org.a.Alpha", "org.c.Gamma", "org.b.Beta"]);
        v.sort("app_id", true);
        assert_eq!(ids(&v), ["org.a.Alpha", "org.b.Beta", "org.c.Gamma"]);
    }

    #[test]
    fn selection_counts_and_survives_reload() {
        let mut v = sample();
        assert_eq!(v.update_counts(), (2, 2));
        assert!(v.toggle_checked(0));
        assert!(!v.toggle_checked(1), "no update, nothing to toggle");
        assert!(!v.toggle_checked(99));
        assert_eq!(v.update_counts(), (2, 1));
        assert_eq!(v.selected_updates(), ["org.kde.Platform"]);
        let items = v.items().to_vec();
        v.set_items(
            items
                .into_iter()
                .map(|mut i| {
                    i.is_checked_for_update = i.has_update;
                    i
                })
                .collect(),
        );
        assert_eq!(
            v.update_counts(),
            (2, 1),
            "deselection is kept across reloads"
        );
        assert_eq!(v.set_all_checked(true), vec![0]);
        assert_eq!(v.update_counts(), (2, 2));
    }

    #[test]
    fn sizes() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(265_100_000), "265.1 MB");
        assert_eq!(parse_size_to_bytes("1.5 GB"), 1_610_612_736);
        assert_eq!(parse_size_to_bytes("300 kB"), 307_200);
    }
}
