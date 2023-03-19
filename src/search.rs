use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

#[derive(Clone)]
pub struct SearchResult {
    pub text: String,
    pub action: Option<ResultAction>,
}

#[derive(Clone)]
pub enum ResultAction {
    Lua,
}

pub struct Search {
    matcher: SkimMatcherV2,
    custom_shortcuts: Vec<SearchResult>,
}

struct KeyMatch {
    name: String,
    kind: Option<MatchKind>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
enum MatchKind {
    Exact,
    StartsWith,
    Fuzzy,
}

impl Search {
    pub fn new() -> Self {
        Self {
            matcher: SkimMatcherV2::default(),
            custom_shortcuts: Vec::new(),
        }
    }

    pub fn add_custom_shortcut(&mut self, name: String) {
        self.custom_shortcuts.push(SearchResult {
            text: name,
            action: Some(ResultAction::Lua),
        });
    }

    pub fn search(&self, input: &str) -> Vec<SearchResult> {
        if input.is_empty() {
            return vec![];
        }
        self.mode_search(input)
    }

    fn do_keymatch(name: String, input: &str, fuzzy: bool) -> KeyMatch {
        let exact_match = name.trim().to_lowercase() == input.trim().to_lowercase();
        let starts_with = name
            .trim()
            .to_lowercase()
            .starts_with(&input.trim().to_lowercase());

        let match_kind = if exact_match {
            Some(MatchKind::Exact)
        } else if starts_with {
            Some(MatchKind::StartsWith)
        } else if fuzzy {
            Some(MatchKind::Fuzzy)
        } else {
            None
        };

        KeyMatch {
            name,
            kind: match_kind,
        }
    }

    fn mode_search(&self, input: &str) -> Vec<SearchResult> {
        let mut vec: Vec<KeyMatch> = Vec::new();

        for custom_shortcut in &self.custom_shortcuts {
            let name = custom_shortcut.text.clone();
            let fuzzy = self
                .matcher
                .fuzzy_match(&name.to_lowercase(), &input.to_lowercase())
                .is_some();

            let km = Self::do_keymatch(name, input, fuzzy);
            vec.push(km);
        }

        let mut available_shortcuts: Vec<&KeyMatch> =
            vec.iter().filter(|x| x.kind.is_some()).collect();

        available_shortcuts.sort_by_cached_key(|x| x.kind);

        available_shortcuts
            .iter()
            .map(|k| {
                let name = &k.name;

                SearchResult {
                    text: name.to_string(),
                    action: Some(ResultAction::Lua),
                }
            })
            .collect()
    }
}
