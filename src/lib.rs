use bevy::{ecs::system::SystemParam, prelude::*};
use uuid::Uuid;

const DEFAULT_MATERIAL_ID: Uuid = Uuid::from_u128(0xb4c3caf5ead145b985d10d8a5fc676d5_u128);

pub mod prelude;
pub mod utils;

mod quad;

/// Holds the resources and systems necessary for a [Sprite3d] to work.
pub struct Sprite3dPlugin;

impl Plugin for Sprite3dPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Sprite3dBillboard>()
            .register_type::<BillboardAssetLoaded>()
            .add_event::<BillboardAssetLoaded>()
            .register_type::<Billboard>()
            .init_asset::<Billboard>()
            .add_systems(Startup, setup)
            .add_systems(
                PostUpdate,
                (handle_texture_atlases, finish_billboards_loading),
            );
    }
}

#[derive(Event, Default, Reflect)]
pub struct BillboardAssetLoaded;

fn setup(mut standard_materials: ResMut<Assets<StandardMaterial>>, mut commands: Commands) {
    standard_materials.insert(
        AssetId::Uuid {
            uuid: DEFAULT_MATERIAL_ID,
        },
        utils::material(),
    );
    commands.spawn((
        Name::new("BillboardAssetsObserver"),
        children![
            Observer::new(update_billboard_mesh::<BillboardAssetLoaded, ()>),
            Observer::new(update_billboard_mesh::<OnInsert, Sprite3dBillboard>)
        ],
    ));
}

// Update the mesh of a Sprite3d with an texture atlas when its index changes.
fn handle_texture_atlases(
    billboards: Res<Assets<Billboard>>,
    mut query: Query<
        (&mut Mesh3d, Option<&Sprite3dAtlasIndex>, &Sprite3dBillboard),
        Or<(Changed<Sprite3dAtlasIndex>, Changed<Sprite3dBillboard>)>,
    >,
) {
    for (mut component_mesh, atlas, billboard_3d) in query.iter_mut() {
        let Some(billboard) = billboards.get(&**billboard_3d) else {
            continue;
        };
        match &billboard.kind {
            BillboardKind::Single { mesh } => {
                **component_mesh = mesh.clone();
            }
            BillboardKind::Atlas { mesh_list, .. } => {
                let Some(texture_atlas) = atlas else {
                    continue;
                };
                if let Some(mesh) = mesh_list.get(**texture_atlas) {
                    **component_mesh = mesh.clone();
                }
            }
        }
    }
}

/// Represents a index of Texture Atlas used by billboard.
#[derive(Clone, Default, Component, Reflect, Debug, Deref, DerefMut)]
#[reflect(Component)]
#[require(Sprite3dBillboard)]
pub struct Sprite3dAtlasIndex(pub usize);

impl From<&TextureAtlas> for Sprite3dAtlasIndex {
    fn from(value: &TextureAtlas) -> Self {
        Self(value.index)
    }
}

// Defines whether the billboard stores a single image or a texture atlas.
#[derive(Clone, Reflect, Debug)]
enum BillboardKind {
    Single {
        mesh: Handle<Mesh>,
    },
    Atlas {
        mesh_list: Vec<Handle<Mesh>>,
        layout: Handle<TextureAtlasLayout>,
    },
}

#[derive(Clone, Component, Deref, Default, Reflect)]
#[require(Transform, MeshMaterial3d<StandardMaterial> = set_material())]
/// Holds the [Billboard] associated with a [Sprite3d]. Has no effect if inserted
/// into an entity without the `Sprite3d` component. The inner `Handle<Billboard>` is
/// private to prevent direct modification, but can be read through dereference.
///
/// An internal system will update the [Mesh3d] and [MeshMaterial3d] components after
/// this component is inserted, once the [Image] associated with the `Billboard` is
/// fully loaded.
pub struct Sprite3dBillboard(Handle<Billboard>);

impl Sprite3dBillboard {
    /// Create a new `Sprite3dBillboard`.
    pub fn new(billboard: Handle<Billboard>) -> Self {
        Self::from(billboard)
    }
}

impl From<Handle<Billboard>> for Sprite3dBillboard {
    fn from(handle: Handle<Billboard>) -> Self {
        Self(handle)
    }
}

fn set_material() -> MeshMaterial3d<StandardMaterial> {
    MeshMaterial3d(Handle::Weak(AssetId::Uuid {
        uuid: DEFAULT_MATERIAL_ID,
    }))
}

#[derive(SystemParam)]
pub struct Billboards<'w> {
    pub billboards: Res<'w, Assets<Billboard>>,
    pub layouts: Res<'w, Assets<TextureAtlasLayout>>,
}

impl Billboards<'_> {
    pub fn textures_len(&self, id: AssetId<Billboard>) -> Option<usize> {
        let Some(billboard) = self.billboards.get(id) else {
            return None;
        };
        match &billboard.kind {
            BillboardKind::Single { .. } => Some(1),
            BillboardKind::Atlas {
                mesh_list: _,
                layout,
            } => {
                if let Some(ll) = self.layouts.get(layout.id()) {
                    Some(ll.textures.len())
                } else {
                    None
                }
            }
        }
    }
}

/// Represents the "billboard", a flat rectangular 3D mesh that the sprite is
/// displayed on. Attached onto a sprite with the [Sprite3dBillboard] component.
#[derive(Clone, Asset, Reflect, Debug)]
pub struct Billboard {
    // The image associated with the billboard.
    #[dependency]
    image: Handle<Image>,
    // See [BillboardKind].
    kind: BillboardKind,
    pixels_per_metre: f32,
    pivot: Vec2,
    double_sided: bool,
    material: Handle<StandardMaterial>,
}

impl Billboard {
    /// Creates a billboard associated with a single image.
    ///
    /// * `image`: The handle to the image associated with this billboard,
    ///   loaded or not.
    /// * `pixels_per_metre`: The number of pixels per metre of the sprite,
    ///   assuming a `Transform::scale` of `1.0`. Defaults to `100.0`.
    /// * `pivot`: The point around which the sprite will rotate. Defaults to the center
    ///   of the image, `Vec2(0.5, 0.5)`.
    /// * `double_sided`: Whether the billboard displays the image on both sides or
    ///   just the 'front'. Defaults to `true`.
    pub fn new(
        image: Handle<Image>,
        pixels_per_metre: f32,
        pivot: Option<Vec2>,
        double_sided: bool,
    ) -> Self {
        Self {
            image,
            pixels_per_metre,
            pivot: pivot.unwrap_or(Vec2::splat(0.5)),
            double_sided,
            ..default()
        }
    }

    /// Creates a billboard associated with a texture atlas. Refer to [Billboard::new]
    /// for more details.
    pub fn with_texture_atlas(
        image: Handle<Image>,
        layout: Handle<TextureAtlasLayout>,
        pixels_per_metre: f32,
        pivot: Option<Vec2>,
        double_sided: bool,
    ) -> Self {
        Self {
            image,
            kind: BillboardKind::Atlas {
                mesh_list: Vec::new(),
                layout,
            },
            pixels_per_metre,
            pivot: pivot.unwrap_or(Vec2::splat(0.5)),
            double_sided,
            ..default()
        }
    }

    pub fn try_get_mesh(
        &self,
        optional_index: Option<&Sprite3dAtlasIndex>,
    ) -> Option<Handle<Mesh>> {
        match &self.kind {
            BillboardKind::Single { mesh } => Some(mesh.clone()),
            BillboardKind::Atlas { mesh_list, .. } => {
                let Some(index) = optional_index else {
                    return None;
                };
                if let Some(mesh) = mesh_list.get(**index) {
                    Some(mesh.clone())
                } else {
                    None
                }
            }
        }
    }
}

impl Default for Billboard {
    fn default() -> Self {
        Self {
            image: Default::default(),
            kind: BillboardKind::Single {
                mesh: Default::default(),
            },
            pixels_per_metre: 100.,
            pivot: Vec2::splat(0.5),
            double_sided: true,
            material: Handle::Weak(AssetId::Uuid {
                uuid: DEFAULT_MATERIAL_ID,
            }),
        }
    }
}

impl From<Handle<Image>> for Billboard {
    fn from(image: Handle<Image>) -> Self {
        Self { image, ..default() }
    }
}

impl From<(Handle<Image>, Handle<TextureAtlasLayout>)> for Billboard {
    fn from((image, layout): (Handle<Image>, Handle<TextureAtlasLayout>)) -> Self {
        Self {
            image,
            kind: BillboardKind::Atlas {
                mesh_list: Vec::new(),
                layout,
            },
            ..default()
        }
    }
}

fn finish_billboards_loading(
    mut asset_events: EventReader<AssetEvent<Billboard>>,
    layouts: Res<Assets<TextureAtlasLayout>>,
    images: Res<Assets<Image>>,
    mut billboards: ResMut<Assets<Billboard>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    q: Query<(Entity, &Sprite3dBillboard)>,
    mut commands: Commands,
) {
    for e in asset_events.read() {
        let id = match e {
            AssetEvent::Added { id } => id,
            // AssetEvent::Modified { id } => id,
            _ => continue,
        };
        let Some(billboard) = billboards.get_mut(*id) else {
            continue;
        };
        let Some(image) = images.get(&billboard.image) else {
            continue;
        };
        let image_size = image.texture_descriptor.size;
        match &billboard.kind {
            BillboardKind::Single { mesh: _ } => {
                // w & h are the world-space size of the sprite
                let w = (image_size.width as f32) / billboard.pixels_per_metre;
                let h = (image_size.height as f32) / billboard.pixels_per_metre;

                let new_mesh = quad::quad(w, h, billboard.pivot, billboard.double_sided);
                let mesh_handle = meshes.add(new_mesh.clone());
                billboard.kind = BillboardKind::Single { mesh: mesh_handle };
            }
            BillboardKind::Atlas {
                mesh_list: _,
                layout: layout_handle,
            } => {
                let layout = layouts.get(layout_handle).unwrap();
                billboard.kind = BillboardKind::Atlas {
                    mesh_list: layout
                        .textures
                        .iter()
                        .map(|rect| {
                            let w = rect.width() as f32 / billboard.pixels_per_metre;
                            let h = rect.height() as f32 / billboard.pixels_per_metre;

                            let frac_rect = bevy::math::Rect {
                                min: Vec2::new(
                                    rect.min.x as f32 / (image_size.width as f32),
                                    rect.min.y as f32 / (image_size.height as f32),
                                ),
                                max: Vec2::new(
                                    rect.max.x as f32 / (image_size.width as f32),
                                    rect.max.y as f32 / (image_size.height as f32),
                                ),
                            };

                            // scale pivot to be relative to the rect within the atlas
                            let mut rect_pivot = billboard.pivot;
                            rect_pivot.x *= frac_rect.width();
                            rect_pivot.y *= frac_rect.height();
                            rect_pivot += frac_rect.min;

                            let mut mesh =
                                quad::quad(w, h, billboard.pivot, billboard.double_sided);
                            mesh.insert_attribute(
                                Mesh::ATTRIBUTE_UV_0,
                                vec![
                                    [frac_rect.min.x, frac_rect.max.y],
                                    [frac_rect.max.x, frac_rect.max.y],
                                    [frac_rect.min.x, frac_rect.min.y],
                                    [frac_rect.max.x, frac_rect.min.y],
                                    [frac_rect.min.x, frac_rect.max.y],
                                    [frac_rect.max.x, frac_rect.max.y],
                                    [frac_rect.min.x, frac_rect.min.y],
                                    [frac_rect.max.x, frac_rect.min.y],
                                ],
                            );

                            meshes.add(mesh)
                        })
                        .collect(),
                    layout: layout_handle.clone(),
                };
            }
        }
        billboard.material = materials.add(StandardMaterial {
            base_color_texture: billboard.image.clone().into(),
            ..utils::material()
        });
        // error!("billboard: {:#?}", &billboard);
        let ids: Vec<Entity> = q
            .iter()
            .flat_map(
                |(e, handle)| {
                    if id.eq(&handle.id()) {
                        Some(e)
                    } else {
                        None
                    }
                },
            )
            .collect();
        commands.trigger_targets(BillboardAssetLoaded, ids);
    }
}

fn update_billboard_mesh<T: Event, B: Bundle>(
    trigger: Trigger<T, B>,
    mut q: Query<(
        Entity,
        &Sprite3dBillboard,
        Option<&Sprite3dAtlasIndex>,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
    billboards: Res<Assets<Billboard>>,
    mut commands: Commands,
) {
    let Ok((e, handle, atlas, mut material)) = q.get_mut(trigger.target()) else {
        return;
    };

    let Some(billboard) = billboards.get(&**handle) else {
        return;
    };
    material.set_if_neq(billboard.material.clone().into());

    let Some(mesh) = billboard.try_get_mesh(atlas) else {
        return;
    };
    commands.entity(e).insert(Mesh3d(mesh));
}
