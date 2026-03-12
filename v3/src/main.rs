use gloo_net::http::Request;
use leptos::prelude::*;
use rexie::{ObjectStore, Rexie, TransactionMode};
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::{spawn_local, JsFuture};

const DB_NAME: &str = "todo_tree_v3";
const STORE_NAME: &str = "app_state";
const STATE_KEY: &str = "root";
const DRIVE_FILE_NAME: &str = "xavier-test-tree-all-projects.json";
const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";

fn default_expanded() -> bool {
    true
}

fn default_show_details() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
enum TaskStatus {
    #[serde(rename = "TODO", alias = "Todo")]
    Todo,
    #[serde(rename = "DONE", alias = "Done")]
    Done,
    #[serde(rename = "IDEA", alias = "Idea")]
    Idea,
    #[serde(rename = "BLOCKED", alias = "Blocked")]
    Blocked,
}

impl TaskStatus {
    fn as_label(&self) -> &'static str {
        match self {
            Self::Todo => "TODO",
            Self::Done => "DONE",
            Self::Idea => "IDEA",
            Self::Blocked => "BLOCKED",
        }
    }

    fn from_str(value: &str) -> Self {
        match value.trim().to_uppercase().as_str() {
            "DONE" => Self::Done,
            "IDEA" => Self::Idea,
            "BLOCKED" => Self::Blocked,
            _ => Self::Todo,
        }
    }
}

fn default_status() -> TaskStatus {
    TaskStatus::Todo
}

fn default_status_options() -> Vec<TaskStatus> {
    vec![TaskStatus::Todo, TaskStatus::Done, TaskStatus::Idea, TaskStatus::Blocked]
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct TodoNode {
    id: String,
    label: String,
    #[serde(default)]
    checked: bool,
    #[serde(default = "default_expanded")]
    expanded: bool,
    #[serde(default = "default_show_details")]
    show_details: bool,
    #[serde(default = "default_status", deserialize_with = "deserialize_status")]
    status: TaskStatus,
    #[serde(default, deserialize_with = "deserialize_tags")]
    tags: Vec<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    links: Vec<String>,
    #[serde(default)]
    children: Vec<TodoNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct ProjectMeta {
    name: String,
    code: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
struct DriveConfig {
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct ProjectTree {
    code: String,
    tree: Vec<TodoNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct AppState {
    #[serde(default)]
    projects: Vec<ProjectMeta>,
    #[serde(default)]
    active_project_code: String,
    #[serde(default)]
    trees: Vec<ProjectTree>,
    #[serde(default)]
    drive: DriveConfig,
    #[serde(default = "default_status_options")]
    available_statuses: Vec<TaskStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredState {
    id: String,
    state: AppState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportPayload {
    version: u32,
    exported_at: String,
    projects: Vec<ExportProject>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ExportProject {
    name: String,
    code: String,
    tree: Vec<TodoNode>,
}

impl Default for AppState {
    fn default() -> Self {
        let default_project = ProjectMeta {
            name: "Default".into(),
            code: "default".into(),
        };

        Self {
            active_project_code: default_project.code.clone(),
            projects: vec![default_project.clone()],
            trees: vec![ProjectTree {
                code: default_project.code,
                tree: default_tree(),
            }],
            drive: DriveConfig {
                status: "Déconnecté".into(),
                ..Default::default()
            },
            available_statuses: default_status_options(),
        }
    }
}

fn uid() -> String {
    let n = (js_sys::Math::random() * 1_000_000_000_000.0) as u64;
    format!("{:x}", n)
}

fn now_iso() -> String {
    js_sys::Date::new_0().to_iso_string().into()
}

fn normalize_project_name(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn project_name_to_code(name: &str) -> String {
    normalize_project_name(name)
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn is_valid_project_name(name: &str) -> bool {
    name.chars().any(|c| c.is_ascii_alphanumeric())
}

fn normalize_status_value(value: &str) -> TaskStatus {
    TaskStatus::from_str(value)
}

fn parse_tags(tags: &str) -> Vec<String> {
    tags.split(|c| c == ',' || c == ';' || c == '\n')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn tags_to_text(tags: &[String]) -> String {
    tags.join(", ")
}

fn deserialize_status<'de, D>(deserializer: D) -> Result<TaskStatus, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    Ok(TaskStatus::from_str(&raw))
}

fn deserialize_tags<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum TagsField {
        Text(String),
        List(Vec<String>),
    }

    let raw = TagsField::deserialize(deserializer)?;
    Ok(match raw {
        TagsField::Text(text) => parse_tags(&text),
        TagsField::List(list) => parse_tags(&tags_to_text(&list)),
    })
}

fn parse_links_text(raw: &str) -> Vec<String> {
    raw.lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn links_to_text(links: &[String]) -> String {
    links.join("\n")
}

fn node_matches_text(node: &TodoNode, term: &str) -> bool {
    if term.is_empty() {
        return true;
    }

    let haystacks = [
        node.label.to_lowercase(),
        node.status.as_label().to_lowercase(),
        tags_to_text(&node.tags).to_lowercase(),
        node.description.to_lowercase(),
        node.links.join(" ").to_lowercase(),
    ];

    haystacks.iter().any(|h| h.contains(term))
}

fn node_matches_tag(node: &TodoNode, tag_filter: &str) -> bool {
    let filters = parse_tags(tag_filter);
    if filters.is_empty() {
        return true;
    }

    let node_tags = node
        .tags
        .iter()
        .map(|t| t.to_lowercase())
        .collect::<Vec<_>>();

    filters.into_iter().all(|filter_tag| {
        let needle = filter_tag.to_lowercase();
        node_tags.iter().any(|tag| tag.contains(&needle))
    })
}

fn normalize_node(node: &mut TodoNode) {
    if node.label.trim().is_empty() {
        node.label = String::new();
    }
    node.tags = parse_tags(&tags_to_text(&node.tags));
    node.links = node
        .links
        .iter()
        .map(|link| link.trim().to_string())
        .filter(|link| !link.is_empty())
        .collect();
    for child in &mut node.children {
        normalize_node(child);
    }
}

fn normalize_state(mut state: AppState) -> AppState {
    if state.projects.is_empty() {
        return AppState::default();
    }

    state.available_statuses = if state.available_statuses.is_empty() {
        default_status_options()
    } else {
        state
            .available_statuses
            .iter()
            .cloned()
            .fold(Vec::<TaskStatus>::new(), |mut acc, status| {
                if !acc.contains(&status) {
                    acc.push(status);
                }
                acc
            })
    };

    for project_tree in &mut state.trees {
        for node in &mut project_tree.tree {
            normalize_node(node);
        }
    }

    if state
        .projects
        .iter()
        .all(|project| project.code != state.active_project_code)
    {
        state.active_project_code = state
            .projects
            .first()
            .map(|project| project.code.clone())
            .unwrap_or_else(|| "default".into());
    }

    state
}

fn create_node(label: &str) -> TodoNode {
    TodoNode {
        id: uid(),
        label: label.into(),
        checked: false,
        expanded: true,
        show_details: true,
        status: default_status(),
        tags: vec![],
        description: String::new(),
        links: vec![],
        children: vec![],
    }
}

fn default_tree() -> Vec<TodoNode> {
    vec![TodoNode {
        id: uid(),
        label: "Lorem".into(),
        checked: false,
        expanded: true,
        show_details: true,
        status: TaskStatus::Todo,
        tags: vec!["migration".into(), "ui".into()],
        description: "Exemple de tâche racine avec détails éditables.".into(),
        links: vec!["https://docs.openclaw.ai".into()],
        children: vec![TodoNode {
            id: uid(),
            label: "Ipsum".into(),
            checked: false,
            expanded: true,
            show_details: true,
            status: TaskStatus::Idea,
            tags: vec!["prototype".into()],
            description: String::new(),
            links: vec![],
            children: vec![],
        }],
    }]
}

fn clone_node_deep_with_new_ids(node: &TodoNode) -> TodoNode {
    TodoNode {
        id: uid(),
        label: node.label.clone(),
        checked: node.checked,
        expanded: node.expanded,
        show_details: node.show_details,
        status: node.status.clone(),
        tags: node.tags.clone(),
        description: node.description.clone(),
        links: node.links.clone(),
        children: node.children.iter().map(clone_node_deep_with_new_ids).collect(),
    }
}

fn markdown_checkbox(checked: bool) -> &'static str {
    if checked { "[X]" } else { "[ ]" }
}

fn markdown_meta(node: &TodoNode) -> String {
    let mut parts = vec![format!("({})", node.status.as_label())];
    if !node.tags.is_empty() {
        parts.push(node.tags.iter().map(|tag| format!("#{}", tag)).collect::<Vec<_>>().join(" "));
    }
    parts.join(" ")
}

fn export_tree_to_markdown(project_name: &str, tree: &[TodoNode]) -> String {
    fn render_children(nodes: &[TodoNode], depth: usize) -> String {
        nodes.iter()
            .map(|node| {
                let indent = "  ".repeat(depth);
                let line = format!(
                    "{}- {} {} {}",
                    indent,
                    markdown_checkbox(node.checked),
                    if node.label.is_empty() { "Sans titre" } else { &node.label },
                    markdown_meta(node)
                );
                let description = if node.description.trim().is_empty() {
                    String::new()
                } else {
                    format!("\n{}  > {}", indent, node.description.replace('\n', " "))
                };
                let links = if node.links.is_empty() {
                    String::new()
                } else {
                    node.links
                        .iter()
                        .map(|link| format!("\n{}  - lien: {}", indent, link))
                        .collect::<String>()
                };
                let children = if node.children.is_empty() {
                    String::new()
                } else {
                    format!("\n{}", render_children(&node.children, depth + 1))
                };
                format!("{}{}{}{}", line, description, links, children)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    let sections = tree
        .iter()
        .map(|root| {
            let title = format!(
                "## {} {} {}",
                markdown_checkbox(root.checked),
                if root.label.is_empty() { "Sans titre" } else { &root.label },
                markdown_meta(root)
            );
            let description = if root.description.trim().is_empty() {
                String::new()
            } else {
                format!("\n\n> {}", root.description.replace('\n', " "))
            };
            let links = if root.links.is_empty() {
                String::new()
            } else {
                format!(
                    "\n\n{}",
                    root.links
                        .iter()
                        .map(|link| format!("- lien: {}", link))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            };
            let children = if root.children.is_empty() {
                String::new()
            } else {
                format!("\n\n{}", render_children(&root.children, 0))
            };
            format!("{}{}{}{}", title, description, links, children)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!("# TODO {}\n\n{}", project_name, sections)
}

fn count_stats(nodes: &[TodoNode]) -> (usize, usize, usize) {
    fn walk(nodes: &[TodoNode], total: &mut usize, done: &mut usize) {
        for node in nodes {
            *total += 1;
            if node.checked {
                *done += 1;
            }
            walk(&node.children, total, done);
        }
    }

    let mut total = 0;
    let mut done = 0;
    walk(nodes, &mut total, &mut done);
    let percent = if total == 0 { 0 } else { (done * 100) / total };
    (total, done, percent)
}

fn set_checked_deep(node: &mut TodoNode, checked: bool) {
    node.checked = checked;
    for child in &mut node.children {
        set_checked_deep(child, checked);
    }
}

fn set_expanded_deep(node: &mut TodoNode, expanded: bool) {
    node.expanded = expanded;
    for child in &mut node.children {
        set_expanded_deep(child, expanded);
    }
}

fn set_all_expanded(nodes: &mut [TodoNode], expanded: bool) {
    for node in nodes {
        set_expanded_deep(node, expanded);
    }
}

fn set_show_details_deep(node: &mut TodoNode, show_details: bool) {
    node.show_details = show_details;
    for child in &mut node.children {
        set_show_details_deep(child, show_details);
    }
}

fn set_all_show_details(nodes: &mut [TodoNode], show_details: bool) {
    for node in nodes {
        set_show_details_deep(node, show_details);
    }
}

fn update_node_by_id<F>(nodes: &mut [TodoNode], node_id: &str, updater: &mut F) -> bool
where
    F: FnMut(&mut TodoNode),
{
    for node in nodes {
        if node.id == node_id {
            updater(node);
            return true;
        }
        if update_node_by_id(&mut node.children, node_id, updater) {
            return true;
        }
    }
    false
}

fn add_child_by_id(nodes: &mut [TodoNode], node_id: &str, new_node: TodoNode) -> bool {
    let mut maybe = Some(new_node);
    update_node_by_id(nodes, node_id, &mut |node| {
        node.expanded = true;
        if let Some(item) = maybe.take() {
            node.children.push(item);
        }
    })
}

fn insert_sibling_by_id(nodes: &mut Vec<TodoNode>, node_id: &str, new_node: TodoNode) -> bool {
    if let Some(index) = nodes.iter().position(|node| node.id == node_id) {
        nodes.insert(index + 1, new_node);
        return true;
    }
    for node in nodes.iter_mut() {
        if insert_sibling_by_id(&mut node.children, node_id, new_node.clone()) {
            return true;
        }
    }
    false
}

fn duplicate_node_as_sibling_by_id(nodes: &mut Vec<TodoNode>, node_id: &str) -> bool {
    if let Some(index) = nodes.iter().position(|node| node.id == node_id) {
        let clone = clone_node_deep_with_new_ids(&nodes[index]);
        nodes.insert(index + 1, clone);
        return true;
    }
    for node in nodes.iter_mut() {
        if duplicate_node_as_sibling_by_id(&mut node.children, node_id) {
            return true;
        }
    }
    false
}

fn remove_node_by_id(nodes: &mut Vec<TodoNode>, node_id: &str) -> bool {
    if let Some(index) = nodes.iter().position(|node| node.id == node_id) {
        nodes.remove(index);
        return true;
    }
    for node in nodes.iter_mut() {
        if remove_node_by_id(&mut node.children, node_id) {
            return true;
        }
    }
    false
}

fn move_item_in_array<T>(items: &mut Vec<T>, from_index: usize, to_index: usize) {
    if from_index >= items.len() || to_index >= items.len() || from_index == to_index {
        return;
    }
    let item = items.remove(from_index);
    items.insert(to_index, item);
}

fn move_node_by_id(nodes: &mut Vec<TodoNode>, node_id: &str, direction: &str) -> bool {
    if let Some(index) = nodes.iter().position(|node| node.id == node_id) {
        match direction {
            "up" if index > 0 => move_item_in_array(nodes, index, index - 1),
            "down" if index + 1 < nodes.len() => move_item_in_array(nodes, index, index + 1),
            _ => return false,
        }
        return true;
    }
    for node in nodes.iter_mut() {
        if move_node_by_id(&mut node.children, node_id, direction) {
            return true;
        }
    }
    false
}

fn can_move_node(nodes: &[TodoNode], node_id: &str, direction: &str) -> bool {
    if let Some(index) = nodes.iter().position(|node| node.id == node_id) {
        return match direction {
            "up" => index > 0,
            "down" => index + 1 < nodes.len(),
            _ => false,
        };
    }
    for node in nodes {
        if can_move_node(&node.children, node_id, direction) {
            return true;
        }
    }
    false
}

fn filter_nodes(nodes: &[TodoNode], text_filter: &str, tag_filter: &str) -> Vec<TodoNode> {
    let text_filter = text_filter.trim().to_lowercase();
    let tag_filter = tag_filter.trim().to_string();

    if text_filter.is_empty() && tag_filter.is_empty() {
        return nodes.to_vec();
    }

    fn walk(nodes: &[TodoNode], text_filter: &str, tag_filter: &str) -> Vec<TodoNode> {
        let mut out = Vec::new();
        for node in nodes {
            let filtered_children = walk(&node.children, text_filter, tag_filter);
            let self_matches = node_matches_text(node, text_filter) && node_matches_tag(node, tag_filter);
            let has_matching_children = !filtered_children.is_empty();

            if self_matches || has_matching_children {
                let mut cloned = node.clone();
                cloned.expanded = if has_matching_children {
                    true
                } else {
                    node.expanded
                };
                cloned.children = if cloned.expanded {
                    filtered_children
                } else {
                    vec![]
                };
                out.push(cloned);
            }
        }
        out
    }

    walk(nodes, &text_filter, &tag_filter)
}

fn active_project<'a>(state: &'a AppState) -> Option<&'a ProjectMeta> {
    state.projects.iter().find(|project| project.code == state.active_project_code)
}

fn active_tree(state: &AppState) -> Vec<TodoNode> {
    state
        .trees
        .iter()
        .find(|tree| tree.code == state.active_project_code)
        .map(|tree| tree.tree.clone())
        .unwrap_or_else(default_tree)
}

fn with_active_tree_mut<R>(state: &mut AppState, f: impl FnOnce(&mut Vec<TodoNode>) -> R) -> R {
    let code = state.active_project_code.clone();
    if let Some(index) = state.trees.iter().position(|tree| tree.code == code) {
        f(&mut state.trees[index].tree)
    } else {
        state.trees.push(ProjectTree {
            code,
            tree: default_tree(),
        });
        let last_index = state.trees.len() - 1;
        f(&mut state.trees[last_index].tree)
    }
}

fn export_all_projects(state: &AppState) -> String {
    let payload = ExportPayload {
        version: 2,
        exported_at: now_iso(),
        projects: state
            .projects
            .iter()
            .map(|project| ExportProject {
                name: project.name.clone(),
                code: project.code.clone(),
                tree: state
                    .trees
                    .iter()
                    .find(|tree| tree.code == project.code)
                    .map(|tree| tree.tree.clone())
                    .unwrap_or_else(default_tree),
            })
            .collect(),
    };
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".into())
}

fn parse_all_projects_json(raw: &str) -> Result<Vec<ExportProject>, String> {
    let parsed: ExportPayload = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    Ok(parsed.projects)
}

async fn open_db() -> Result<Rexie, JsValue> {
    Rexie::builder(DB_NAME)
        .version(1)
        .add_object_store(ObjectStore::new(STORE_NAME).key_path("id"))
        .build()
        .await
        .map_err(|e| JsValue::from_str(&format!("IndexedDB open error: {e:?}")))
}

async fn load_state() -> Result<AppState, JsValue> {
    let rexie = open_db().await?;
    let tx = rexie
        .transaction(&[STORE_NAME], TransactionMode::ReadOnly)
        .map_err(|e| JsValue::from_str(&format!("IndexedDB tx error: {e:?}")))?;
    let store = tx
        .store(STORE_NAME)
        .map_err(|e| JsValue::from_str(&format!("IndexedDB store error: {e:?}")))?;

    let maybe = store
        .get(JsValue::from_str(STATE_KEY))
        .await
        .map_err(|e| JsValue::from_str(&format!("IndexedDB get error: {e:?}")))?;

    match maybe {
        Some(value) => {
            let json = js_sys::JSON::stringify(&value)?
                .as_string()
                .ok_or_else(|| JsValue::from_str("Failed to stringify DB value"))?;
            let wrapper: StoredState = serde_json::from_str(&json)
                .map_err(|e| JsValue::from_str(&format!("Deserialize error: {e}")))?;
            Ok(normalize_state(wrapper.state))
        }
        None => Ok(AppState::default()),
    }
}

async fn save_state(state: &AppState) -> Result<(), JsValue> {
    let rexie = open_db().await?;
    let tx = rexie
        .transaction(&[STORE_NAME], TransactionMode::ReadWrite)
        .map_err(|e| JsValue::from_str(&format!("IndexedDB tx error: {e:?}")))?;
    let store = tx
        .store(STORE_NAME)
        .map_err(|e| JsValue::from_str(&format!("IndexedDB store error: {e:?}")))?;

    let wrapper = StoredState {
        id: STATE_KEY.to_string(),
        state: normalize_state(state.clone()),
    };

    let value = serde_wasm_bindgen::to_value(&wrapper)
        .map_err(|e| JsValue::from_str(&format!("Serialize error: {e}")))?;

    store
        .put(&value, None)
        .await
        .map_err(|e| JsValue::from_str(&format!("IndexedDB put error: {e:?}")))?;

    tx.done()
        .await
        .map_err(|e| JsValue::from_str(&format!("IndexedDB commit error: {e:?}")))?;
    Ok(())
}

fn prompt(message: &str, default: &str) -> Option<String> {
    web_sys::window()
        .and_then(|window| window.prompt_with_message_and_default(message, default).ok())
        .flatten()
}

fn confirm(message: &str) -> bool {
    web_sys::window()
        .and_then(|window| window.confirm_with_message(message).ok())
        .unwrap_or(false)
}

fn download_text_file(filename: &str, content: &str, mime: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("document unavailable"))?;
    let array = js_sys::Array::new();
    array.push(&JsValue::from_str(content));
    let bag = web_sys::BlobPropertyBag::new();
    bag.set_type(mime);
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&array, &bag)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let anchor = document
        .create_element("a")?
        .dyn_into::<web_sys::HtmlAnchorElement>()?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    web_sys::Url::revoke_object_url(&url)?;
    Ok(())
}

async fn drive_find_file(token: &str) -> Result<Option<String>, String> {
    let url = format!(
        "https://www.googleapis.com/drive/v3/files?spaces=appDataFolder&q=name='{}' and trashed=false&fields=files(id,name,modifiedTime,size)",
        DRIVE_FILE_NAME
    );
    let text = Request::get(&url)
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(value["files"]
        .as_array()
        .and_then(|files| files.first())
        .and_then(|file| file["id"].as_str())
        .map(|s| s.to_string()))
}

async fn drive_download_file(token: &str, file_id: &str) -> Result<String, String> {
    Request::get(&format!(
        "https://www.googleapis.com/drive/v3/files/{}?alt=media",
        file_id
    ))
    .header("Authorization", &format!("Bearer {token}"))
    .send()
    .await
    .map_err(|e| e.to_string())?
    .text()
    .await
    .map_err(|e| e.to_string())
}

async fn request_google_drive_token(client_id: &str) -> Result<String, String> {
    let window = web_sys::window().ok_or_else(|| "window unavailable".to_string())?;
    let func = js_sys::Reflect::get(&window, &JsValue::from_str("requestGoogleDriveToken"))
        .map_err(|_| "Fonction requestGoogleDriveToken introuvable".to_string())?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| "requestGoogleDriveToken n'est pas une fonction".to_string())?;
    let promise = func
        .call2(&window, &JsValue::from_str(client_id), &JsValue::from_str(DRIVE_SCOPE))
        .map_err(|e| format!("Erreur OAuth: {:?}", e))?
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| "OAuth n'a pas renvoyé de Promise".to_string())?;
    let token = JsFuture::from(promise)
        .await
        .map_err(|e| format!("Erreur Google Identity Services: {:?}", e))?;
    token
        .as_string()
        .ok_or_else(|| "Token Google invalide".to_string())
}

async fn drive_upload_file(token: &str, payload: &str, existing_file_id: Option<String>) -> Result<(), String> {
    let is_update = existing_file_id.is_some();
    let metadata = if is_update {
        serde_json::json!({ "name": DRIVE_FILE_NAME })
    } else {
        serde_json::json!({ "name": DRIVE_FILE_NAME, "parents": ["appDataFolder"] })
    };

    let form = web_sys::FormData::new().map_err(|e| format!("FormData: {:?}", e))?;

    let metadata_parts = js_sys::Array::new();
    metadata_parts.push(&JsValue::from_str(&metadata.to_string()));
    let metadata_bag = web_sys::BlobPropertyBag::new();
    metadata_bag.set_type("application/json");
    let metadata_blob = web_sys::Blob::new_with_str_sequence_and_options(&metadata_parts, &metadata_bag)
        .map_err(|e| format!("Blob metadata: {:?}", e))?;
    form.append_with_blob("metadata", &metadata_blob)
        .map_err(|e| format!("Form metadata: {:?}", e))?;

    let file_parts = js_sys::Array::new();
    file_parts.push(&JsValue::from_str(payload));
    let file_bag = web_sys::BlobPropertyBag::new();
    file_bag.set_type("application/json");
    let file_blob = web_sys::Blob::new_with_str_sequence_and_options(&file_parts, &file_bag)
        .map_err(|e| format!("Blob file: {:?}", e))?;
    form.append_with_blob("file", &file_blob)
        .map_err(|e| format!("Form file: {:?}", e))?;

    let endpoint = if let Some(file_id) = existing_file_id {
        format!(
            "https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=multipart&fields=id,name,modifiedTime",
            file_id
        )
    } else {
        "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&fields=id,name,modifiedTime".to_string()
    };

    let options = web_sys::RequestInit::new();
    options.set_method(if is_update { "PATCH" } else { "POST" });
    options.set_body(&form);

    let headers = web_sys::Headers::new().map_err(|e| format!("Headers: {:?}", e))?;
    headers
        .set("Authorization", &format!("Bearer {token}"))
        .map_err(|e| format!("Authorization header: {:?}", e))?;
    options.set_headers(&headers);

    let request = web_sys::Request::new_with_str_and_init(&endpoint, &options)
        .map_err(|e| format!("Request: {:?}", e))?;
    let window = web_sys::window().ok_or_else(|| "window unavailable".to_string())?;
    let response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch: {:?}", e))?
        .dyn_into::<web_sys::Response>()
        .map_err(|_| "Réponse fetch invalide".to_string())?;

    if response.ok() {
        Ok(())
    } else {
        let text = JsFuture::from(response.text().map_err(|e| format!("Response text: {:?}", e))?)
            .await
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_else(|| "Erreur Drive inconnue".into());
        Err(text)
    }
}

fn status_badge_class(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Done => "status-badge status-done",
        TaskStatus::Idea => "status-badge status-idea",
        TaskStatus::Blocked => "status-badge status-blocked",
        TaskStatus::Todo => "status-badge status-todo",
    }
}

fn render_project_chip(
    project: ProjectMeta,
    state: RwSignal<AppState>,
    mode: String,
    status_signal: RwSignal<String>,
) -> AnyView {
    let code = project.code.clone();
    let name = project.name.clone();
    let select_code = code.clone();

    let actions = if mode == "edit" {
        let rename_code = code.clone();
        let delete_code = code.clone();
        view! {
            <>
                <button class="secondary tiny" on:click=move |_| {
                    let current_name = state
                        .get_untracked()
                        .projects
                        .iter()
                        .find(|project| project.code == rename_code)
                        .map(|project| project.name.clone())
                        .unwrap_or_default();
                    if let Some(next_name_raw) = prompt("Renommer le projet", &current_name) {
                        let next_name = normalize_project_name(&next_name_raw);
                        let next_code = project_name_to_code(&next_name);
                        if !is_valid_project_name(&next_name) || next_code.is_empty() {
                            status_signal.set("Nom de projet invalide".into());
                            return;
                        }
                        if state
                            .get_untracked()
                            .projects
                            .iter()
                            .any(|project| project.code == next_code && project.code != rename_code)
                        {
                            status_signal.set("Un projet avec ce code existe déjà".into());
                            return;
                        }
                        state.update(|app| {
                            if let Some(project) = app.projects.iter_mut().find(|project| project.code == rename_code) {
                                project.name = next_name.clone();
                                project.code = next_code.clone();
                            }
                            if let Some(tree) = app.trees.iter_mut().find(|tree| tree.code == rename_code) {
                                tree.code = next_code.clone();
                            }
                            if app.active_project_code == rename_code {
                                app.active_project_code = next_code.clone();
                            }
                        });
                    }
                }>"✏️"</button>
                <button class="danger tiny" on:click=move |_| {
                    let project_name = state
                        .get_untracked()
                        .projects
                        .iter()
                        .find(|project| project.code == delete_code)
                        .map(|project| project.name.clone())
                        .unwrap_or_else(|| delete_code.clone());
                    if !confirm(&format!("Supprimer le projet \"{}\" ?", project_name)) {
                        return;
                    }
                    state.update(|app| {
                        app.projects.retain(|project| project.code != delete_code);
                        app.trees.retain(|tree| tree.code != delete_code);
                        if app.projects.is_empty() {
                            *app = AppState::default();
                        } else if app.active_project_code == delete_code {
                            app.active_project_code = app.projects[0].code.clone();
                        }
                    });
                }>"🗑"</button>
            </>
        }
        .into_any()
    } else {
        view! { <></> }.into_any()
    };

    view! {
        <div class="row" style="gap:4px;">
            <button
                class:active=move || state.get().active_project_code == code
                on:click=move |_| {
                    state.update(|app| app.active_project_code = select_code.clone());
                }
            >
                {name}
            </button>
            {actions}
        </div>
    }
    .into_any()
}

fn render_status_editor(
    current_status: TaskStatus,
    status_options: Vec<TaskStatus>,
    state: RwSignal<AppState>,
    node_id: String,
) -> AnyView {
    let mut options = status_options;
    if !options.contains(&current_status) {
        options.push(current_status.clone());
    }

    view! {
        <select class="status-select" on:change=move |ev| {
            let value = event_target_select_value(&ev);
            state.update(|app| {
                with_active_tree_mut(app, |tree| {
                    update_node_by_id(tree, &node_id, &mut |node| node.status = normalize_status_value(&value));
                });
            });
        }>
            {options.into_iter().map(|status| {
                let selected = status == current_status;
                let value = status.as_label().to_string();
                let label = value.clone();
                view! {
                    <option value=value.clone() selected=selected>{label}</option>
                }
            }).collect_view()}
        </select>
    }
    .into_any()
}

fn render_link_list(links: &[String]) -> AnyView {
    if links.is_empty() {
        view! { <></> }.into_any()
    } else {
        let link_views = links
            .iter()
            .map(|link| {
                let href = link.clone();
                let label = link.clone();
                view! {
                    <a class="link-chip" href=href target="_blank" rel="noopener noreferrer">{label}</a>
                }
            })
            .collect_view();
        view! { <div class="row links-row">{link_views}</div> }.into_any()
    }
}

fn render_tags(tags: &[String]) -> AnyView {
    if tags.is_empty() {
        view! { <></> }.into_any()
    } else {
        let tags_view = tags
            .iter()
            .cloned()
            .map(|tag| view! { <span class="tag-badge">{tag}</span> })
            .collect_view();
        view! { <div class="row tags-row">{tags_view}</div> }.into_any()
    }
}

fn render_node(
    node: TodoNode,
    state: RwSignal<AppState>,
    mode: String,
    depth: usize,
    status_options: Vec<TaskStatus>,
) -> AnyView {
    let has_children = !node.children.is_empty();
    let links_text = links_to_text(&node.links);
    let can_up = can_move_node(&active_tree(&state.get_untracked()), &node.id, "up");
    let can_down = can_move_node(&active_tree(&state.get_untracked()), &node.id, "down");
    let node_id = node.id.clone();
    let expand_id = node_id.clone();
    let left_id = node_id.clone();
    let label_id = node_id.clone();
    let add_child_id = node_id.clone();
    let add_sibling_id = node_id.clone();
    let dup_id = node_id.clone();
    let del_id = node_id.clone();
    let up_id = node_id.clone();
    let down_id = node_id.clone();
    let tags_id = node_id.clone();
    let description_id = node_id.clone();
    let links_id = node_id.clone();
    let details_id = node_id.clone();

    let left_control = if mode == "view" {
        view! {
            <input type="checkbox" prop:checked=node.checked on:change=move |ev| {
                let checked = event_target_checked(&ev);
                state.update(|app| {
                    with_active_tree_mut(app, |tree| {
                        update_node_by_id(tree, &left_id, &mut |node| set_checked_deep(node, checked));
                    });
                });
            } />
        }
        .into_any()
    } else {
        view! { <span class="badge depth-badge">{depth}</span> }.into_any()
    };

    let label_view = if mode == "edit" {
        let initial_value = node.label.clone();
        view! {
            <input class="grow" type="text" value=initial_value on:input=move |ev| {
                let value = event_target_value(&ev);
                state.update(|app| {
                    with_active_tree_mut(app, |tree| {
                        update_node_by_id(tree, &label_id, &mut |node| node.label = value.clone());
                    });
                });
            } />
        }
        .into_any()
    } else {
        let class_name = if node.checked { "done grow title-text" } else { "grow title-text" };
        view! { <span class=class_name>{if node.label.is_empty() { "Sans titre".to_string() } else { node.label.clone() }}</span> }.into_any()
    };

    let meta_view = if !node.show_details {
        view! { <></> }.into_any()
    } else if mode == "edit" {
        let tag_value = tags_to_text(&node.tags);
        let description_value = node.description.clone();
        let links_value = links_text.clone();
        view! {
            <div class="editor-block">
                <div class="row compact-row">
                    {render_status_editor(node.status.clone(), status_options.clone(), state, node_id.clone())}
                    <input class="grow" type="text" value=tag_value placeholder="tags métier: backend, qa, urgent" on:input=move |ev| {
                        let value = event_target_value(&ev);
                        state.update(|app| {
                            with_active_tree_mut(app, |tree| {
                                update_node_by_id(tree, &tags_id, &mut |node| node.tags = parse_tags(&value));
                            });
                        });
                    } />
                </div>
                <textarea class="detail-textarea" placeholder="Description / détails" on:input=move |ev| {
                    let value = event_target_value(&ev);
                    state.update(|app| {
                        with_active_tree_mut(app, |tree| {
                            update_node_by_id(tree, &description_id, &mut |node| node.description = value.clone());
                        });
                    });
                }>{description_value}</textarea>
                <textarea class="detail-textarea" placeholder="Liens (un par ligne)" on:input=move |ev| {
                    let value = event_target_value(&ev);
                    state.update(|app| {
                        with_active_tree_mut(app, |tree| {
                            update_node_by_id(tree, &links_id, &mut |node| node.links = parse_links_text(&value));
                        });
                    });
                }>{links_value}</textarea>
            </div>
        }
        .into_any()
    } else {
        let status_badge = view! { <span class=status_badge_class(&node.status)>{node.status.as_label().to_string()}</span> };
        let description_view = if node.description.trim().is_empty() {
            view! { <></> }.into_any()
        } else {
            view! { <p class="node-description">{node.description.clone()}</p> }.into_any()
        };

        view! {
            <div class="meta-stack grow">
                <div class="row compact-row">
                    {status_badge}
                    {render_tags(&node.tags)}
                </div>
                {description_view}
                {render_link_list(&node.links)}
            </div>
        }
        .into_any()
    };

    let actions = if mode == "edit" {
        view! {
            <div class="row action-row">
                <button class="secondary tiny" on:click=move |_| {
                    state.update(|app| {
                        with_active_tree_mut(app, |tree| {
                            add_child_by_id(tree, &add_child_id, create_node("Nouvel enfant"));
                        });
                    });
                }>"+ Child"</button>
                <button class="secondary tiny" on:click=move |_| {
                    state.update(|app| {
                        with_active_tree_mut(app, |tree| {
                            insert_sibling_by_id(tree, &add_sibling_id, create_node("Nouveau sibling"));
                        });
                    });
                }>"+ Sibling"</button>
                <button class="secondary tiny" on:click=move |_| {
                    state.update(|app| {
                        with_active_tree_mut(app, |tree| {
                            duplicate_node_as_sibling_by_id(tree, &dup_id);
                        });
                    });
                }>"Dupliquer"</button>
                <button class="secondary tiny" on:click=move |_| {
                    state.update(|app| {
                        with_active_tree_mut(app, |tree| {
                            move_node_by_id(tree, &up_id, "up");
                        });
                    });
                } disabled=!can_up>"↑"</button>
                <button class="secondary tiny" on:click=move |_| {
                    state.update(|app| {
                        with_active_tree_mut(app, |tree| {
                            move_node_by_id(tree, &down_id, "down");
                        });
                    });
                } disabled=!can_down>"↓"</button>
                <button class="danger tiny" on:click=move |_| {
                    state.update(|app| {
                        with_active_tree_mut(app, |tree| {
                            remove_node_by_id(tree, &del_id);
                        });
                    });
                }>"Supprimer"</button>
            </div>
        }
        .into_any()
    } else {
        view! { <></> }.into_any()
    };

    let children_view = if node.expanded && has_children {
        let child_views = node
            .children
            .clone()
            .into_iter()
            .map(|child| render_node(child, state, mode.clone(), depth + 1, status_options.clone()))
            .collect_view();
        view! { <div class="children">{child_views}</div> }.into_any()
    } else {
        view! { <></> }.into_any()
    };

    view! {
        <div class="node-wrap" style=format!("margin-left:{}px", depth * 10)>
            <div class="node-card">
                <div class="row compact-row">
                    <button class="secondary tiny" on:click=move |_| {
                        state.update(|app| {
                            with_active_tree_mut(app, |tree| {
                                update_node_by_id(tree, &expand_id, &mut |node| node.expanded = !node.expanded);
                            });
                        });
                    } disabled=!has_children>
                        {if node.expanded { "−" } else { "+" }}
                    </button>
                    {left_control}
                    {label_view}
                    <button class="secondary tiny" title=if node.show_details { "Compacter" } else { "Décompacter" } on:click=move |_| {
                        state.update(|app| {
                            with_active_tree_mut(app, |tree| {
                                update_node_by_id(tree, &details_id, &mut |node| node.show_details = !node.show_details);
                            });
                        });
                    }>
                        {if node.show_details { "👁" } else { "🙈" }}
                    </button>
                    {actions}
                </div>
                {meta_view}
            </div>
            {children_view}
        </div>
    }
    .into_any()
}

#[component]
fn App() -> impl IntoView {
    let state = RwSignal::new(AppState::default());
    let mode = RwSignal::new(String::from("view"));
    let text_filter = RwSignal::new(String::new());
    let tag_filter = RwSignal::new(String::new());
    let status_message = RwSignal::new(String::from("Chargement..."));
    let new_project_name = RwSignal::new(String::new());
    let current_json = RwSignal::new(String::new());
    let markdown = RwSignal::new(String::new());
    let all_projects_json = RwSignal::new(String::new());
    let drive_client_id = RwSignal::new(String::new());
    let drive_token_input = RwSignal::new(String::new());

    Effect::new(move |_| {
        let state = state;
        let status_message = status_message;
        let current_json = current_json;
        let markdown = markdown;
        let all_projects_json = all_projects_json;
        spawn_local(async move {
            match load_state().await {
                Ok(loaded) => {
                    let normalized = normalize_state(loaded);
                    let active_tree_snapshot = active_tree(&normalized);
                    let project_name = active_project(&normalized)
                        .map(|project| project.name.clone())
                        .unwrap_or_else(|| "Default".into());
                    current_json.set(serde_json::to_string_pretty(&active_tree_snapshot).unwrap_or_default());
                    markdown.set(export_tree_to_markdown(&project_name, &active_tree_snapshot));
                    all_projects_json.set(export_all_projects(&normalized));
                    drive_client_id.set(normalized.drive.client_id.clone());
                    drive_token_input.set(normalized.drive.access_token.clone());
                    state.set(normalized);
                    status_message.set("État chargé depuis IndexedDB".into());
                }
                Err(err) => status_message.set(format!("Chargement impossible: {:?}", err)),
            }
        });
    });

    Effect::new(move |_| {
        let snapshot = normalize_state(state.get());
        let active_tree_snapshot = active_tree(&snapshot);
        let project_name = active_project(&snapshot)
            .map(|project| project.name.clone())
            .unwrap_or_else(|| "Default".into());
        current_json.set(serde_json::to_string_pretty(&active_tree_snapshot).unwrap_or_default());
        markdown.set(export_tree_to_markdown(&project_name, &active_tree_snapshot));
        all_projects_json.set(export_all_projects(&snapshot));
    });

    Effect::new(move |_| {
        let snapshot = normalize_state(state.get());
        let status_message = status_message;
        spawn_local(async move {
            match save_state(&snapshot).await {
                Ok(()) => status_message.set("Synchronisé dans IndexedDB".into()),
                Err(err) => status_message.set(format!("Erreur de sauvegarde: {:?}", err)),
            }
        });
    });

    Effect::new(move |_| {
        if let Some(window) = web_sys::window() {
            let sw = window.navigator().service_worker();
            let _ = sw.register("/sw.js");
        }
    });

    let on_add_root = move |_| {
        state.update(|app| {
            with_active_tree_mut(app, |tree| tree.push(create_node("Nouvelle section")));
        });
    };

    let on_expand_all = move |_| {
        state.update(|app| {
            with_active_tree_mut(app, |tree| set_all_expanded(tree, true));
        });
    };

    let on_collapse_all = move |_| {
        state.update(|app| {
            with_active_tree_mut(app, |tree| set_all_expanded(tree, false));
        });
    };

    let on_expand_details_all = move |_| {
        state.update(|app| {
            with_active_tree_mut(app, |tree| set_all_show_details(tree, true));
        });
    };

    let on_compact_all = move |_| {
        state.update(|app| {
            with_active_tree_mut(app, |tree| set_all_show_details(tree, false));
        });
    };

    let on_create_project = move |_| {
        let name = normalize_project_name(&new_project_name.get_untracked());
        let code = project_name_to_code(&name);
        if !is_valid_project_name(&name) || code.is_empty() {
            status_message.set("Nom de projet invalide".into());
            return;
        }
        state.update(|app| {
            if app.projects.iter().any(|project| project.code == code) {
                return;
            }
            app.projects.push(ProjectMeta {
                name: name.clone(),
                code: code.clone(),
            });
            app.trees.push(ProjectTree {
                code: code.clone(),
                tree: default_tree(),
            });
            app.active_project_code = code;
        });
        new_project_name.set(String::new());
    };

    let on_import_current = move |_| {
        let raw = current_json.get_untracked();
        match serde_json::from_str::<Vec<TodoNode>>(&raw) {
            Ok(mut tree) => {
                for node in &mut tree {
                    normalize_node(node);
                }
                state.update(|app| {
                    with_active_tree_mut(app, |active_tree| *active_tree = tree.clone());
                });
            }
            Err(err) => status_message.set(format!("JSON projet invalide: {}", err)),
        }
    };

    let on_import_all = move |_| {
        let raw = all_projects_json.get_untracked();
        match parse_all_projects_json(&raw) {
            Ok(imported) => {
                state.update(|app| {
                    for mut project in imported {
                        for node in &mut project.tree {
                            normalize_node(node);
                        }
                        if !app.projects.iter().any(|p| p.code == project.code) {
                            app.projects.push(ProjectMeta {
                                name: project.name.clone(),
                                code: project.code.clone(),
                            });
                        } else if let Some(meta) = app.projects.iter_mut().find(|p| p.code == project.code) {
                            meta.name = project.name.clone();
                        }
                        if let Some(tree) = app.trees.iter_mut().find(|tree| tree.code == project.code) {
                            tree.tree = project.tree.clone();
                        } else {
                            app.trees.push(ProjectTree {
                                code: project.code.clone(),
                                tree: project.tree.clone(),
                            });
                        }
                    }
                });
            }
            Err(err) => status_message.set(format!("JSON global invalide: {}", err)),
        }
    };

    let on_download_json = move |_| {
        let filename = format!(
            "checklist-tree-{}.json",
            active_project(&state.get_untracked())
                .map(|project| project.code.clone())
                .unwrap_or_else(|| "project".into())
        );
        let _ = download_text_file(&filename, &current_json.get_untracked(), "application/json");
    };

    let on_download_all = move |_| {
        let _ = download_text_file(
            "checklist-tree-all-projects.json",
            &all_projects_json.get_untracked(),
            "application/json",
        );
    };

    let on_download_md = move |_| {
        let filename = format!(
            "todo-{}.md",
            active_project(&state.get_untracked())
                .map(|project| project.code.clone())
                .unwrap_or_else(|| "project".into())
        );
        let _ = download_text_file(&filename, &markdown.get_untracked(), "text/markdown");
    };

    let on_connect_drive = move |_| {
        let client_id = drive_client_id.get_untracked();
        if client_id.trim().is_empty() {
            status_message.set("Client ID Google manquant".into());
            return;
        }
        let status_signal = status_message;
        let state_signal = state;
        spawn_local(async move {
            match request_google_drive_token(&client_id).await {
                Ok(token) => {
                    drive_token_input.set(token.clone());
                    state_signal.update(|app| {
                        app.drive.client_id = client_id.clone();
                        app.drive.access_token = token.clone();
                        app.drive.status = "Connecté".into();
                        app.drive.message = "Connexion Google Drive OK".into();
                    });
                    status_signal.set("Connexion Google Drive OK".into());
                }
                Err(err) => {
                    state_signal.update(|app| {
                        app.drive.client_id = client_id.clone();
                        app.drive.status = "Erreur".into();
                        app.drive.message = err.clone();
                    });
                    status_signal.set("Connexion Google Drive impossible".into());
                }
            }
        });
    };

    let on_check_drive = move |_| {
        let token = drive_token_input.get_untracked();
        if token.trim().is_empty() {
            status_message.set("Token Google Drive manquant".into());
            return;
        }
        state.update(|app| {
            app.drive.client_id = drive_client_id.get_untracked();
            app.drive.access_token = token.clone();
        });
        let status_signal = status_message;
        let state_signal = state;
        spawn_local(async move {
            match drive_find_file(&token).await {
                Ok(Some(file_id)) => {
                    state_signal.update(|app| {
                        app.drive.status = "Connecté".into();
                        app.drive.message = format!("Fichier Drive trouvé: {}", file_id);
                    });
                    status_signal.set("Google Drive joignable".into());
                }
                Ok(None) => {
                    state_signal.update(|app| {
                        app.drive.status = "Connecté".into();
                        app.drive.message = "Aucun fichier de synchro trouvé".into();
                    });
                    status_signal.set("Google Drive joignable, fichier absent".into());
                }
                Err(err) => {
                    state_signal.update(|app| {
                        app.drive.status = "Erreur".into();
                        app.drive.message = err.clone();
                    });
                    status_signal.set(format!("Échec Google Drive: {}", err));
                }
            }
        });
    };

    let on_import_drive = move |_| {
        let token = drive_token_input.get_untracked();
        if token.trim().is_empty() {
            status_message.set("Token Google Drive manquant".into());
            return;
        }
        let state_signal = state;
        let status_signal = status_message;
        spawn_local(async move {
            match drive_find_file(&token).await {
                Ok(Some(file_id)) => match drive_download_file(&token, &file_id).await {
                    Ok(raw) => match parse_all_projects_json(&raw) {
                        Ok(imported) => {
                            state_signal.update(|app| {
                                for mut project in imported {
                                    for node in &mut project.tree {
                                        normalize_node(node);
                                    }
                                    if !app.projects.iter().any(|p| p.code == project.code) {
                                        app.projects.push(ProjectMeta {
                                            name: project.name.clone(),
                                            code: project.code.clone(),
                                        });
                                    } else if let Some(meta) = app.projects.iter_mut().find(|p| p.code == project.code) {
                                        meta.name = project.name.clone();
                                    }
                                    if let Some(tree) = app.trees.iter_mut().find(|tree| tree.code == project.code) {
                                        tree.tree = project.tree.clone();
                                    } else {
                                        app.trees.push(ProjectTree {
                                            code: project.code.clone(),
                                            tree: project.tree.clone(),
                                        });
                                    }
                                }
                            });
                            status_signal.set("Import Drive effectué".into());
                        }
                        Err(err) => status_signal.set(format!("JSON Drive invalide: {}", err)),
                    },
                    Err(err) => status_signal.set(format!("Téléchargement Drive impossible: {}", err)),
                },
                Ok(None) => status_signal.set("Aucun fichier Drive à importer".into()),
                Err(err) => status_signal.set(format!("Échec Google Drive: {}", err)),
            }
        });
    };

    let on_upload_drive = move |_| {
        let token = drive_token_input.get_untracked();
        if token.trim().is_empty() {
            status_message.set("Token Google Drive manquant".into());
            return;
        }
        let payload = export_all_projects(&normalize_state(state.get_untracked()));
        let state_signal = state;
        let status_signal = status_message;
        spawn_local(async move {
            match drive_find_file(&token).await {
                Ok(existing_file_id) => match drive_upload_file(&token, &payload, existing_file_id).await {
                    Ok(()) => {
                        state_signal.update(|app| {
                            app.drive.status = "Connecté".into();
                            app.drive.message = "Upload Drive effectué".into();
                        });
                        status_signal.set("Upload Drive effectué".into());
                    }
                    Err(err) => status_signal.set(format!("Upload Drive impossible: {}", err)),
                },
                Err(err) => status_signal.set(format!("Échec Google Drive: {}", err)),
            }
        });
    };

    let filtered_tree = Signal::derive(move || {
        let snapshot = normalize_state(state.get());
        filter_nodes(&active_tree(&snapshot), &text_filter.get(), &tag_filter.get())
    });

    let stats = Signal::derive(move || {
        let snapshot = normalize_state(state.get());
        count_stats(&active_tree(&snapshot))
    });

    view! {
        <main class="app">
            <section class="panel">
                <div class="row">
                    <h1 class="grow">"Todo Tree v3"</h1>
                    <button class:active=move || mode.get() == "view" on:click=move |_| mode.set("view".into())>"Affichage"</button>
                    <button class:active=move || mode.get() == "edit" on:click=move |_| mode.set("edit".into())>"Édition"</button>
                    <button class:active=move || mode.get() == "settings" on:click=move |_| mode.set("settings".into())>"Settings"</button>
                </div>
                <p class="muted">{move || status_message.get()}</p>
                <p class="muted">
                    {move || {
                        let (total, done, percent) = stats.get();
                        format!("{} éléments • {} cochés • {}%", total, done, percent)
                    }}
                </p>
            </section>

            <section class="panel" style="margin-top:16px;">
                <h2>"Projets"</h2>
                <div class="row" style="margin-bottom:10px;">
                    {move || {
                        let snapshot = normalize_state(state.get());
                        let current_mode = mode.get();
                        snapshot
                            .projects
                            .into_iter()
                            .map(|project| render_project_chip(project, state, current_mode.clone(), status_message))
                            .collect_view()
                    }}
                </div>
                <Show when=move || mode.get() == "edit">
                    <div class="row">
                        <input class="grow" type="text" placeholder="Nouveau projet" prop:value=move || new_project_name.get() on:input=move |ev| new_project_name.set(event_target_value(&ev)) />
                        <button on:click=on_create_project>"+ Projet"</button>
                    </div>
                </Show>
            </section>

            <section class="panel" style="margin-top:16px;">
                <h2>"Statuts"</h2>
                <div class="row" style="margin-bottom:10px;">
                    {move || {
                        normalize_state(state.get())
                            .available_statuses
                            .into_iter()
                            .map(|status| view! {
                                <span class=status_badge_class(&status)>{status.as_label().to_string()}</span>
                            })
                            .collect_view()
                    }}
                </div>
                <p class="muted">"Statuts disponibles : TODO, DONE, IDEA, BLOCKED."</p>
            </section>

            <section class="panel" style="margin-top:16px;">
                <div class="row">
                    <input class="grow" type="text" placeholder="Recherche texte..." prop:value=move || text_filter.get() on:input=move |ev| text_filter.set(event_target_value(&ev)) />
                    <input class="grow" type="text" placeholder="Filtre tags..." prop:value=move || tag_filter.get() on:input=move |ev| tag_filter.set(event_target_value(&ev)) />
                    <button class="secondary" on:click=on_expand_all>"Tout déplier"</button>
                    <button class="secondary" on:click=on_collapse_all>"Tout replier"</button>
                    <button class="secondary" on:click=on_compact_all>"Compacter"</button>
                    <button class="secondary" on:click=on_expand_details_all>"Décompacter"</button>
                    <Show when=move || mode.get() == "edit">
                        <button on:click=on_add_root>"+ Racine"</button>
                    </Show>
                </div>
                <div style="margin-top:16px;">
                    {move || {
                        let snapshot = normalize_state(state.get());
                        let current_mode = mode.get();
                        let status_options = snapshot.available_statuses.clone();
                        filtered_tree
                            .get()
                            .into_iter()
                            .map(|node| render_node(node, state, current_mode.clone(), 0, status_options.clone()))
                            .collect_view()
                    }}
                </div>
            </section>

            <Show when=move || mode.get() != "settings">
                <section class="panel" style="margin-top:16px;">
                    <h2>"Markdown"</h2>
                    <textarea class="code" prop:value=move || markdown.get() readonly=true></textarea>
                    <div class="row" style="margin-top:10px;">
                        <button class="secondary" on:click=on_download_md>"Télécharger Markdown"</button>
                    </div>
                </section>
            </Show>

            <Show when=move || mode.get() == "settings">
                <section class="panel" style="margin-top:16px;">
                    <h2>"JSON projet actif"</h2>
                    <textarea class="code" prop:value=move || current_json.get() on:input=move |ev| current_json.set(event_target_value(&ev))></textarea>
                    <div class="row" style="margin-top:10px;">
                        <button on:click=on_import_current>"Importer JSON"</button>
                        <button class="secondary" on:click=on_download_json>"Télécharger JSON"</button>
                    </div>
                </section>

                <section class="panel" style="margin-top:16px;">
                    <h2>"Tous les projets"</h2>
                    <textarea class="code" prop:value=move || all_projects_json.get() on:input=move |ev| all_projects_json.set(event_target_value(&ev))></textarea>
                    <div class="row" style="margin-top:10px;">
                        <button on:click=on_import_all>"Importer tous les projets"</button>
                        <button class="secondary" on:click=on_download_all>"Télécharger tout"</button>
                    </div>
                </section>

                <section class="panel" style="margin-top:16px; margin-bottom:24px;">
                    <h2>"Google Drive"</h2>
                    <p class="muted">"Client ID OAuth + token + import/upload du fichier global dans appDataFolder."</p>
                    <div class="row" style="margin-bottom:10px;">
                        <input class="grow" type="text" placeholder="Google OAuth Client ID" prop:value=move || drive_client_id.get() on:input=move |ev| drive_client_id.set(event_target_value(&ev)) />
                        <button on:click=on_connect_drive>"Connecter Drive"</button>
                    </div>
                    <input class="grow" type="text" placeholder="Access token Google Drive" prop:value=move || drive_token_input.get() on:input=move |ev| drive_token_input.set(event_target_value(&ev)) />
                    <div class="row" style="margin-top:10px;">
                        <button on:click=on_check_drive>"Vérifier"</button>
                        <button class="secondary" on:click=on_import_drive>"Importer depuis Drive"</button>
                        <button class="secondary" on:click=on_upload_drive>"Uploader vers Drive"</button>
                    </div>
                    <p class="muted" style="margin-top:10px;">
                        {move || {
                            let drive = normalize_state(state.get()).drive;
                            format!("{} — {}", drive.status, drive.message)
                        }}
                    </p>
                </section>
            </Show>
        </main>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App)
}

fn event_target_value<T>(ev: &T) -> String
where
    T: AsRef<web_sys::Event>,
{
    ev.as_ref()
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.value())
        .or_else(|| {
            ev.as_ref()
                .target()
                .and_then(|target| target.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
                .map(|textarea| textarea.value())
        })
        .unwrap_or_default()
}

fn event_target_checked<T>(ev: &T) -> bool
where
    T: AsRef<web_sys::Event>,
{
    ev.as_ref()
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.checked())
        .unwrap_or(false)
}

fn event_target_select_value<T>(ev: &T) -> String
where
    T: AsRef<web_sys::Event>,
{
    ev.as_ref()
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlSelectElement>().ok())
        .map(|select| select.value())
        .unwrap_or_default()
}
