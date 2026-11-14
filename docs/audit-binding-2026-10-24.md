# Audit du binding C — 24 octobre 2026

- **Commit audité** : `8e66cdb` (master, post-fusion PR #5 — wgpu 30.0.1)
- **Méthode** : 6 audits indépendants en sous-agents (conv enums/bitflags ; conv descripteurs ; device/pipelines ; encodeurs/passes/queue ; instance/adapter/surface ; socle transverse objets/futures/erreurs/buffers), croisés avec les headers vendus (`ffi/ffi/{webgpu.h,wgpu.h,wgpu_upstream.h}`, contrat ABI source wgpu-native v29.0.1.1), le source local wgpu-core/wgpu-types/wgpu-hal **30.0.1** (`~/.rust/cargo/registry/src/...`), et la référence comportementale wgpu-native v29.0.1.1.
- **Verdict global** : le socle est solide — aucune faille de sûreté mémoire, référencement Arc et protocole des futures validés de bout en bout — mais **11 bugs réels**, dont 4 paniquent sur des entrées C valides selon le header et 3 altèrent silencieusement des données. L'immense majorité est héritée du portage wgpu-native v29 ; un seul point (n° 8, UB sur `hinstance` NULL) semble propre au packer.

---

## 🔴 Bugs à corriger

### 1. `to_native_texture_usage_flags` : `COPY_DST` mappe vers `CopySrc`
`conv.rs:1885-1887` — copié-collé de la ligne `COPY_SRC` :
```rust
if flags.contains(wgt::TextureUsages::COPY_DST) {
    flag |= native::WGPUTextureUsage_CopySrc;   // devrait être CopyDst
}
```
Consommé par `wgpuSurfaceGetCapabilities` (`api/surface.rs:91-92`) : `WGPUTextureUsage_CopyDst` n'est jamais rapporté au C et `CopySrc` l'est à tort. Un consommateur conforme qui teste `usages & CopyDst` avant de configurer la surface conclut à tort que l'usage n'est pas supporté. *Recoupé par 3 agents.* Hérité de wgpu-native v29.

### 2. `map_texture_usage_flags` jette `StorageBinding` et `TransientAttachment`
`conv.rs:1862-1877` — seuls CopySrc/CopyDst/TextureBinding/RenderAttachment sont mappés, alors que le header vendu définit `WGPUTextureUsage_StorageBinding = 0x8` et `TransientAttachment = 0x20` (`webgpu.h:1369-1371`) et que wgpu-types 30 possède les deux bits (`texture.rs:183-186`).

**Changement de sémantique wgpu 30** : un binding de texture storage exige désormais l'usage `STORAGE_BINDING` (`wgpu-core-30.0.1/src/device/resource.rs:3765` : `view.check_usage(wgt::TextureUsages::STORAGE_BINDING)?`). Un client C qui déclare `TextureBinding|StorageBinding` conformément au header obtient une texture sans STORAGE_BINDING → erreur de validation différée à la création du bind group. Le code est identique chez wgpu-native v29, mais celui-ci ciblait wgpu 29 où `TEXTURE_BINDING` suffisait : **le portage vers wgpu 30 rend le drop fonctionnellement faux**. C'est le constat au plus fort impact du portage 29→30.

### 3. `budgetForDeviceLoss` jamais lu
`conv.rs:394-395` :
```rust
let for_resource_creation = unsafe { extras.budgetForDeviceCreation.as_ref() }.copied();
let for_device_loss = unsafe { extras.budgetForDeviceCreation.as_ref() }.copied();  // copié-collé
```
Le header (`wgpu_upstream.h:1170-1171`) définit deux pointeurs distincts → `wgt::MemoryBudgetThresholds` reçoit deux fois le seuil de création. *Recoupé par 3 agents.* Hérité.

### 4. `viewDimension = Undefined` → panic dans les binding layouts
`conv.rs:1722-1733` (texture) et `conv.rs:1758-1768` (storage texture) : `_ => panic!("invalid texture view dimension ...")`. Le header documente « If set to WGPUTextureViewDimension_Undefined, defaults to WGPUTextureViewDimension_2D » (`webgpu.h:2909-2911`) et le macro `WGPU_TEXTURE_BINDING_LAYOUT_INIT` pose `Undefined` : un descripteur initialisé avec les défauts du header panique. Correctif : `Undefined => wgt::TextureViewDimension::D2` dans les deux matchs. Hérité.

### 5. `WGPUVertexFormat_Unorm8x4BGRA` non mappé → panic
`conv.rs:156-200` : la liste (40 variants) omet `Unorm8x4BGRA`, présent dans le header (42 valeurs hors Force32) et dans wgt 30 sous le nom `Unorm8x4Bgra` (`naga-types-30.0.1/src/lib.rs:318`). Le caller (`api/device.rs:487`) fait `.expect("invalid vertex format ...")` → panic sur entrée valide.

### 6. Vertex buffer « trou » valide → panic
`api/device.rs:478-480` : `stepMode=Undefined` + `attributeCount == 0` → `panic!("Unused vertex buffers are currently not supported...")`. Le header (`webgpu.h:4385-4397`) spécifie ce cas comme **valide** (« a "hole" in the parent WGPUVertexState buffers array, with behavior equivalent to null in the JS API ») et wgc 30 le supporte nativement (`VertexState.buffers: Cow<[Option<VertexBufferLayout>]>`). Correctif trivial : émettre `None`.

### 7. `WGPUBindGroupLayoutEntry.bindingArraySize` ignoré
`conv.rs:1806` — seul l'extras legacy `WGPUBindGroupLayoutEntryExtras.count` alimente `wgt::BindGroupLayoutEntry.count`. Le champ standard (`webgpu.h:3631-3634`, « If non-zero, this entry defines a binding array with this size ») n'est jamais lu : un binding array demandé par la voie standard devient silencieusement un binding simple. Correctif : `NonZeroU32::new(entry.bindingArraySize).or_else(|| extras.and_then(...))`.

### 8. Handles de surface nuls → UB (`NonZero*::new_unchecked`)
`conv.rs:1925` (`NonZeroIsize::new_unchecked(win.hinstance)`), et mêmes motifs sur `hwnd` (`:1922`), `xcb.connection` (`:1941-1943`), xlib display (`:1953-1954`), wayland display/surface (`:1964-1965`), android window (`:1973`). Les macros INIT du header mettent ces champs à NULL → `new_unchecked(0)` = UB immédiat, pas même une panic contenue. **Seule divergence introduite par le packer** : wgpu-native v29 utilise la forme vérifiée pour `hinstance` (`src/conv.rs:1877`). Correctif : `NonZero::new(x).expect("null surface source handle")` partout.

### 9. `wgpuDevicePopErrorScope` sur pile vide → panic
`api/device.rs:1112` : `error_sink.scopes.pop().unwrap()`. Le header définit explicitement `WGPUPopErrorScopeStatus_Error = 0x3` (« The error scope stack could not be popped, because it was empty », `webgpu.h:764-768`) : un pop sans push est une séquence C **prévue** qui doit livrer le callback avec ce statut. Hérité, mais le packer a toute la machinerie `future::register/complete` pour le faire proprement.

### 10. Callback de perte de device : tirs multiples + handle potentiellement dangling
`api/device.rs:1005-1040` + `error.rs:99-131` :
- **Tirs multiples** — à la première erreur `DeviceLost`, la lost future est consommée ; mais toute erreur `Lost` **ultérieure** (les opérations post-perte échouent systématiquement avec `DeviceError::Lost`) repasse dans la branche de tir immédiat → le `deviceLostCallback` est appelé une 2ᵉ/3ᵉ fois. Scénario voisin : perte avant tout `GetLostFuture` (tir immédiat), puis `GetLostFuture` qui complète la future et **re-tire** le callback (2 invocations au total).
- **Handle dangling** — `lost_callback_payload(..., self.device.as_ptr() ...)` capture un pointeur depuis un `Weak` sans posséder de référence : en livraison différée (`AllowProcessEvents`/`WaitAnyOnly`) après `wgpuDeviceRelease`, le callback reçoit un `*const WGPUDevice` vers un `WGPUDeviceImpl` libéré — un `wgpuDeviceAddRef` côté client serait un use-after-free.
Correctif : flag `lost_delivered` + capturer un `future::OwnedHandle<WGPUDeviceImpl>` (mécanisme déjà utilisé pour les pipelines/requestDevice).

### 11. `wgpuBufferMapAsync` ignore `WGPU_WHOLE_MAP_SIZE`
`api/buffer.rs:261` : `Some(size as wgt::BufferAddress)` inconditionnel. `size == WGPU_WHOLE_MAP_SIZE` (SIZE_MAX, documenté « defaults to buffer.size - offset », `webgpu.h:6117`) devient `Some(u64::MAX)` → erreur de validation wgc au lieu de mapper jusqu'à la fin. Correctif : `match size { WGPU_WHOLE_MAP_SIZE => None, _ => Some(size as u64) }` (wgc 30 gère `None` = reste du buffer). Hérité.

---

## 🟠 Écarts au contrat

1. **Timed WaitAny : mélange queue + CPU accepté** — `api/instance.rs:430-443`. La spec « Mixed Sources » exige `WGPUWaitStatus_Error` (« waiting on a Queue-timeline future and some other CPU-timeline future is an error ») ; le code ne rejette que `queues.len() > 1`. Le commentaire revendique la conformité sans l'implémenter.
2. **Features avalées en silence** — `conv.rs:1496-1586` : fallback `_ => None` sans warning. (a) 5 constantes standard sans équivalent wgt 30 (`Subgroups`, `TextureComponentSwizzle`, `TextureFormatsTier1/2`, `CoreFeaturesAndLimits`) droppées dans `requiredFeatures` : le client croit la feature activée ; (b) le TODO « requires wgpu.h api change » sur 10 `WGPUNativeFeature_*` est **périmé** — wgt 30 les possède toutes, le mapping pourrait être activé.
3. **`maxBindGroupsPlusVertexBuffers` non relayé** — `conv.rs:542-543` (write) et `:615-618` (read), commentés « TODO: not yet in wgt » alors que `wgt::Limits` 30.0.1 possède le champ : limite silencieusement abandonnée dans les deux sens.
4. **Champs reconnus mais ignorés** — `tracePath` (SType `DeviceExtras` résolu mais champ non relayé, `conv.rs:471-508`) ; `dxcMaxShaderModel` (`conv.rs:372-424`) ; `featureLevel` de `WGPURequestAdapterOptions` (`api/instance.rs:229-273`) ; `subgroupMinSize/subgroupMaxSize` jamais remplis bien que disponibles dans wgt 30 (`conv.rs:1226-1236`).
5. **`zero_initialize_workgroup_memory: false`** — `api/device.rs:218, 465, 563` (TODO documenté) contre le défaut spec `true` (« required by the WebGPU spec » côté wgc 30).
6. **Cycle de vie du device** — `wgpuDeviceDestroy` ne branche jamais `device_set_device_lost_closure` (perte notifiée uniquement quand une opération échoue ensuite) ; `wgpuDevicePoll` pendant une perte → `handle_error_fatal` = abort (balisé FIXME, `api/device.rs:1199-1205`) ; `wgpuDeviceCreateBuffer` ne retourne jamais NULL contrairement au header (« if mappedAtCreation allocation fails, returns NULL ») ; error scopes par device au lieu de « current thread's » (`webgpu.h:6362/6367`).
7. **`wgpuGetVersion()` retourne toujours 0** — `logging.rs:6-25` : `WGPU_NATIVE_VERSION` n'est injecté nulle part ; ajouter `println!("cargo:rustc-env=WGPU_NATIVE_VERSION=30.0.1.0")` dans `ffi/build.rs`.
8. **`NULL + count 0` sur les bundles : UB formel** — `api/render.rs:195-215` (SetBindGroup) et `:861-873` (SetImmediates) passent le pointeur C brut aux fonctions `bundle_ffi` qui font `slice::from_raw_parts(ptr, 0)` sans test de nullité ; les chemins équivalents des passes utilisent `make_slice` qui neutralise ce cas.
9. **Warning mensonger depthSlice** — `api/command.rs:146-148` loggue « Depth slice on color attachments is not implemented » alors que `depth_slice` **est** relayé (`:161-163`) et géré par wgc 30.

## 🟡 Risques (fragile mais fonctionnel, hérité pour l'essentiel)

- Étiquettes de fonction erronées dans les messages d'erreur : `render.rs:433` (`DrawIndirect` étiqueté `DrawIndexedIndirect`), `render.rs:685` (`SetScissorRect` étiqueté `SetPipeline`) ; messages « invalid compute pass encoder » dans les fonctions render pass.
- `SetImmediates` prend `size: u32` au lieu du `size_t` du header (`webgpu.h:6231/6407/6507`) — troncature théorique > 4 Gio.
- Command buffers finis mais jamais soumis : fuite jusqu'à la destruction de l'instance (`objects.rs:82-94`, parité wgpu-native).
- Callbacks `AllowSpontaneous` tirés sous les verrous du sink (`error.rs:91-131`) — ré-entrée C dans l'API sur le même device = deadlock parking_lot (la spec déclare la ré-entrée UB depuis un callback spontané).
- `WGPUInstanceDescriptor.requiredLimits` jamais lu ni validé (`api/instance.rs:30-50`).
- `future::purge` droppe les payloads sous le verrou du registre (`future.rs:232-243`) — un payload possédant un device bloquerait `wgpuInstanceRelease` sur un poll indéfini.
- `WaitAny` peut retourner `TimedOut` pour une future `AllowSpontaneous` complétée sur un autre thread (`notify_wait_scope` est thread-local) — callback quand même livré exactement une fois.
- `AdapterInfo.vendor` reçoit le nom du *driver*, `architecture` vide (`conv.rs:1229`) — hérité.
- Double `complete()` silencieux sur `wgpuBufferMapAsync` en rejet synchrone (wgc appelle la closure ET retourne Err) — tir unique garanti mais fragile.
- `GUIDE.md §5` obsolète : les erreurs uncaptured loggent désormais au lieu d'aborter.
- Handles inter-instance non validés sur `RequestAdapter.compatibleSurface` / `SurfaceGetCapabilities` / `SurfaceConfigure` (le registre des futures, lui, vérifie `owner`).

---

## ✅ Périmètres validés conformes

- **Encodeurs / passes / queue / bundles** (`api/{render,compute,command,queue}.rs`) : **aucun bug**. Arguments wgc 30 vérifiés un à un (`clear_buffer`, `copy_b2b`, `resolve_query_set`, `set_index/set_vertex_buffer(Option<BufferSize>)`, `multi_draw_*`, `queue_write_buffer/texture`...) ; sentinelles `WGPU_WHOLE_SIZE`, `COPY_STRIDE/ARRAY_LAYER/MIP_COUNT/DEPTH_SLICE/QUERY_SET_INDEX_UNDEFINED`, `WGPU_STRLEN` correctes ; `SetVertexBuffer` nullable conforme header ; `Box::from_raw`/`take` du bundle encoder sans double-finish ni fuite ; états begin/end routés vers le error sink (pas de panic sur usage invalide) ; `OnSubmittedWorkDone` = future réelle tirée une fois selon le mode.
- **Référencement Arc des 22 types d'objets** (`objects.rs` + modules api) : `AddRef`/`Release` symétriques des `Arc::into_raw`, drops wgc tolérants aux ids absents, `Fallible::Invalid` enregistré sur échec de création (handles d'erreur libérés proprement), aucune fuite ni double-free détecté.
- **Protocole des futures** (`future.rs`) : ids monotones ≥ 1 ; exactement un tir par future sur tous les chemins (spontané = retrait + tir hors verrou → ré-entrance sûre ; `sweep_process_events` ne livre que `AllowProcessEvents` ; purge ne tire jamais et libère les handles possédés via `OwnedHandle`) ; conformité aux modes de `webgpu.h:456-480`.
- **Helpers chaînes/slices** (`utils.rs`) : sentinelles `{NULL,0}`/`WGPU_STRLEN` conformes, aucun `CString` temporaire dont le pointeur fuirait, vues `Box<str>` ↔ `Box<[u8]>` symétriques.
- **`wgpuInstanceWaitAny`** : conforme clause par clause à la spec « Asynchronous Operations » (futures nulles/doublons/inconnues/étrangères → Error ; TimedWaitAny exigée ; timeout 0 = poll unique ; succès = au moins une complétion, drapeau `completed`, callbacks hors verrou, entrées consommées) — hors l'écart « mixed sources » 🟠 n° 1.
- **Couverture ABI** : 100 % des exports `webgpu.h` + `wgpu_upstream.h` implémentés ; seuls ajout `wgpuDebugTriggerPanic` (sonde debug, `ffi/ffi/wgpu.h`) ; audit symboles 229 déclarées / 0 manquantes / 0 fantômes.

---

## Proposition de suite

Branche `fix/binding-audit` corrigeant les 11 🔴 — presque tous des correctifs d'une à cinq lignes ; trois d'entre eux (trous vertex, `bindingArraySize`, `maxBindGroupsPlusVertexBuffers`) deviennent triviaux grâce à wgc 30 — avec oracles ajoutés pour les cas paniquants (n° 4, 5, 6, 9). Les 🟠 peuvent suivre dans la même branche ou dans une seconde selon l'ampleur souhaitée.
