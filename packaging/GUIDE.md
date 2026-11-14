# Consommer wgpu-packer depuis Kotlin/Native

## 1. Décompresser l'archive

```bash
tar xzf wgpu-<version>-<cible>-static.tar.gz
# → include/ (webgpu.h, wgpu_upstream.h, wgpu.h) · lib/ (libwgpu_packer.a, link.txt) · LICENSE*
```

## 2. cinterop

Recopier `packaging/wgpu.def`, ajuster `libraryPaths` vers `lib/` de l'archive.
Les symboles arrivent dans le package `wgpu` (ex. `wgpu.wgpuCreateInstance`).
Le header à déclarer est `wgpu.h` — il inclut le standard et les extensions
vendored ; ne jamais inclure `wgpu_upstream.h` directement.

## 3. Édition de liens

Les drapeaux exacts par cible sont dans `lib/link.txt` de l'archive — à
recopier dans `linkerOpts` (Gradle) ou la config de lien.
macOS : `-framework Metal -framework QuartzCore -framework CoreVideo
-framework IOKit -framework Foundation` · Linux : `-ldl -lpthread -lm` ·
Android : `-llog -landroid` · Windows : `user32 gdi32 dxgi d3d12`.

## 4. Contrat mémoire (obligatoire)

- Chaque objet obtenu (Instance, Adapter, Device, Buffer…) doit être libéré
  par sa fonction `wgpuXxxRelease` — appariement strict 1:1 ; `wgpuXxxAddRef`
  si vous copiez le handle.
- `wgpuInstanceEnumerateAdapters` : premier appel avec `adapters == NULL` →
  nombre ; second appel avec un tableau de cette taille → remplissage.
  **Chaque adapter retourné doit être Release'd.**
- `wgpuBufferGetMappedRange` n'est valide qu'entre livraison du `map_async` et
  `wgpuBufferUnmap` ; après `unmap` la fonction retourne NULL (jamais de
  crash).
- `MAP_WRITE` se combine avec `COPY_SRC`, `MAP_READ` avec `COPY_DST` (règle
  WebGPU « MAP + COPY opposé ») — toute autre combinaison est une erreur de
  validation.

## 5. Contrat de threading et callbacks

- Les callbacks passent par des structs `WGPU*CallbackInfo`
  (`callback` + `userdata1`/`userdata2`). Mettre le pointeur de fonction dans
  `.callback`, le pointeur de routage (consumé par les trampolines du packer,
  cf. `routingUserdataField: userdata2`) dans `.userdata2`, et le `StableRef`
  Kotlin — le payload applicatif — dans `.userdata1`.
- **Le champ `.mode` est obligatoire** : `AllowSpontaneous` (livraison dès la
  complétion — avant le retour pour une opération résolue synchrone),
  `AllowProcessEvents` (livré au prochain traitement d'événements de
  l'instance) ou `WaitAnyOnly` (livré uniquement par une attente couvrant la
  future). Le macro `INIT` met 0, qui est **invalide** : l'appel panique.
- `wgpuAdapterRequestDevice` et les créations de pipelines async sont
  **synchrones** en interne : avec `AllowSpontaneous`, le callback part avant
  le retour de l'appel.
- `wgpuBufferMapAsync` est résolu au **poll** (poll du device, ou poll
  interne d'un traitement d'événements / d'une attente) : poll AVANT tout
  `Release` de l'objet source ; libérez le `StableRef` (`dispose()`) dans le
  callback même, exactement une fois.
- Toute opération async retourne une **future réelle** (id ≠ 0).
  `wgpuInstanceWaitAny` : `timeoutNS > 0` exige la feature `TimedWaitAny`
  demandée à la création de l'instance (limite rapportée : aucune) et des
  futures d'une seule source (une queue au plus — « Mixed Sources ») ;
  `timeoutNS == 0` = poll unique non bloquant ; (0 future, timeout 0) =
  équivalent de ProcessEvents. Futures inconnues, déjà livrées, nulles ou en
  doublon → `WGPUWaitStatus_Error`. Les callbacks non livrés à la destruction
  de l'instance sont annulés silencieusement.
- Les erreurs GPU non capturées ne sont **jamais abort** : sans handler
  installé, le handler par défaut du packer **journalise et ignore**
  (callbacks.rs : « log sans paniquer »), et les erreurs de validation de
  `wgpuQueueSubmit` comme du `finish` d'un render bundle sont livrées au
  sink du device (scope d'erreur courant ou uncaptured error handler).
  Positionnez `uncapturedErrorCallbackInfo.callback` dans le
  `WGPUDeviceDescriptor` dès le premier device pour **consommer** les erreurs
  au lieu de les laisser au log par défaut.
- La **perte de device** n'est pas une erreur de programmation (reset GPU,
  thermique Android, TDR Windows) : positionnez aussi
  `deviceLostCallbackInfo.callback` sur chaque device pour être notifié et
  pouvoir reconstruire vos ressources. La perte est détectée au moment où
  elle remonte (poll ou erreur), sans thread dédié ; la future de perte
  (`wgpuDeviceGetLostFuture`, id stable par device) suit le mode de ce
  callback.

## 6. Fonctions non implémentées (appel = abort)

Ces symboles sont exportés mais lèvent `unimplemented!()` (panique) à l'appel
en v30.0.1.0 — ne les appelez pas depuis Kotlin. 6 stubs ; les raisons sont
détaillées dans `ffi/src/unimplemented.rs` :

wgpuInstanceGetWGSLLanguageFeatures, wgpuInstanceHasWGSLLanguageFeature,
wgpuSupportedWGSLLanguageFeaturesFreeMembers, wgpuShaderModuleGetCompilationInfo,
wgpuExternalTextureAddRef, wgpuExternalTextureRelease.

À noter :

- Les fonctions `*SetLabel` sont acceptées et ignorées : wgpu-core 29
  n'expose aucun changement de label post-création (cf.
  `ffi/src/api/labels.rs`). Les labels des messages d'erreur restent ceux de
  la création.
- La création asynchrone de pipelines est synchrone en interne (§5) ; les
  erreurs de validation ne passent que par le callback.
- La dimension de vue de binding des textures est résolue côté packer
  (valeur chaînée explicite ou règle de dérivation) — wgpu-core 29 ne
  l'expose pas (gfx-rs/wgpu#7428).

Toute autre fonction déclarée dans webgpu.h/wgpu.h est implémentée — y compris
la résolution de procédures par nom, l'état de mapping des buffers, les
features d'instance, les informations d'adaptateur côté device et les
futures/attentes d'instance (§5).

## 7. Versionnement

`<wgpu-majeur>.<mineur>.<patch>.<packer>` — ex. `30.0.1.0` = wgpu 30.0.1,
révision 0 du packer. Les enums/flags sont réservés pour ray tracing, video
et queries avancées (ajouts sans rupture, spec §5).
