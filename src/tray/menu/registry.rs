use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use getset::Getters;
use tray_icon::menu::{CheckMenuItem, MenuId, MenuItemKind};

#[derive(Clone, Getters)]
#[getset(get = "pub")]
pub struct MenuItemMeta<G> {
    kind: MenuItemKind,
    group: Option<G>,
}

impl<G> PartialEq for MenuItemMeta<G>
where
    G: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.group == other.group && self.kind.id() == other.kind.id()
    }
}

#[derive(Clone, Getters)]
#[getset(get = "pub")]
struct RadioGroup {
    members: HashSet<MenuId>,
    default: Option<MenuId>,
}

#[derive(Clone)]
pub struct MenuRegistry<G>
where
    G: Clone + Copy + Eq + Hash + PartialEq + std::fmt::Debug,
{
    id_to_kind: HashMap<MenuId, MenuItemMeta<G>>,
    radio_groups: HashMap<G, RadioGroup>,
    normal_groups: HashMap<G, HashSet<MenuId>>,
}

impl<G> Default for MenuRegistry<G>
where
    G: Clone + Copy + Eq + Hash + PartialEq + std::fmt::Debug,
{
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl<G> MenuRegistry<G>
where
    G: Clone + Copy + Eq + Hash + PartialEq + std::fmt::Debug,
{
    pub fn new() -> Self {
        Self {
            id_to_kind: HashMap::new(),
            radio_groups: HashMap::new(),
            normal_groups: HashMap::new(),
        }
    }

    pub fn register(&mut self, kind: MenuItemKind, group: Option<G>) {
        let id = kind.id().clone();

        if let Some(group) = group {
            self.normal_groups
                .entry(group)
                .or_default()
                .insert(id.clone());
        }

        self.id_to_kind.insert(id, MenuItemMeta { kind, group });
    }

    pub fn register_radio(
        &mut self,
        kind: MenuItemKind,
        group: G,
        default: Option<MenuId>,
    ) -> bool {
        if kind.as_check_menuitem().is_none() {
            return false;
        }

        let id = kind.id().clone();

        self.id_to_kind.insert(
            id.clone(),
            MenuItemMeta {
                kind,
                group: Some(group),
            },
        );

        self.radio_groups
            .entry(group)
            .or_insert_with(|| RadioGroup {
                members: HashSet::new(),
                default,
            })
            .members
            .insert(id);

        true
    }

    pub fn deregister(&mut self, id: &MenuId, group: Option<G>) -> bool {
        if self.id_to_kind.remove(id).is_none() {
            return false;
        }

        group.is_some_and(|g| {
            self.normal_groups
                .get_mut(&g)
                .map(|ids| ids.remove(id))
                .unwrap_or_default()
        })
    }

    pub fn deregister_radio(&mut self, id: &MenuId, group: G) -> bool {
        if self.id_to_kind.remove(id).is_none() {
            return false;
        }

        self.radio_groups
            .get_mut(&group)
            .map(|radio_group| radio_group.members.remove(id))
            .unwrap_or_default()
    }

    pub fn handle_event(&mut self, id: &MenuId) -> Result<&MenuItemMeta<G>, String> {
        let menu_item_meta = self
            .get_menu_meta_from_id(id)
            .ok_or_else(|| format!("The menu not found: {id:?}"))?;

        let menu_group = menu_item_meta.group();

        // Clicked menu is not in any group, return directly
        let Some(menu_group) = menu_group else {
            return Ok(menu_item_meta);
        };

        let menu_kind = &menu_item_meta.kind();

        // Clicked menu is not in any [Radio] group, return directly
        let Some(radio_group) = self.radio_groups.get(menu_group) else {
            return Ok(menu_item_meta);
        };

        // <---Handle [Radio] menu--->
        let radio_menus_id = radio_group.members();

        let clickd_radio_menu_is_checked = menu_kind
            .as_check_menuitem()
            .ok_or_else(|| {
                format!("Menu({id:?}) is not a [CheckMenuItem] on the radio group({menu_group:?})")
            })?
            .is_checked();

        // Clicked menu is selected, deselect other raodio menus
        if clickd_radio_menu_is_checked {
            radio_menus_id
                .iter()
                .filter(|menu_id| menu_id.ne(&id))
                .filter_map(|id| self.id_to_kind.get(id))
                .filter_map(|menu_meta| menu_meta.kind().as_check_menuitem())
                .for_each(|check_menu| check_menu.set_checked(false));

            Ok(menu_item_meta)
        // Clicked menu is not selected, check if there is a default menu in the group
        } else {
            let Some(default_menu_id) = radio_group.default().as_ref() else {
                // No default menu, return and uncheck all menus
                self.get_radio_menu_from_group(menu_group)
                    .ok_or_else(|| format!("Failed to get radio menus from {menu_group:?}"))?
                    .iter()
                    .for_each(|check_menu| check_menu.set_checked(false));

                return Ok(menu_item_meta);
            };

            let default_menu_meta = self
                .id_to_kind
                .get(default_menu_id)
                .ok_or_else(|| format!("Default menu({default_menu_id:?}) meta not found"))?;

            let default_menu_item = default_menu_meta.kind().as_check_menuitem()
                .ok_or_else(|| format!("Default Menu({default_menu_id:?}) is not a [CheckMenuItem] on the radio group({menu_group:?})"))?;

            // Uncheck all other menus except the default menu
            default_menu_item.set_checked(true);
            radio_menus_id
                .iter()
                .filter(|menu_id| menu_id.ne(&default_menu_id))
                .filter_map(|id| self.id_to_kind.get(id))
                .filter_map(|menu_meta| menu_meta.kind().as_check_menuitem())
                .for_each(|check_menu| check_menu.set_checked(false));

            Ok(default_menu_meta)
        }
    }

    pub fn get_menu_meta_from_id(&self, id: &MenuId) -> Option<&MenuItemMeta<G>> {
        self.id_to_kind.get(id)
    }

    pub fn get_menu_kind_from_id(&self, id: &MenuId) -> Option<&MenuItemKind> {
        self.id_to_kind.get(id).map(|meta| &meta.kind)
    }

    pub fn get_menu_group_from_id(&self, id: &MenuId) -> Option<G> {
        self.id_to_kind.get(id).and_then(|meta| meta.group)
    }

    pub fn get_radio_id_from_group(&self, group: &G) -> Option<&HashSet<MenuId>> {
        self.radio_groups.get(group).map(|r| r.members())
    }

    pub fn get_radio_menu_from_group(&self, group: &G) -> Option<Vec<&CheckMenuItem>> {
        self.get_radio_id_from_group(group).map(|ids| {
            ids.iter()
                .filter_map(|id| self.id_to_kind.get(id))
                .filter_map(|meta| meta.kind.as_check_menuitem())
                .collect::<Vec<_>>()
        })
    }
}
