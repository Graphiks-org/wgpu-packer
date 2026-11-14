# Spike: Android Native (`androidNativeArm64`, `androidNativeX64`)

## Question

Le packer peut-il publier des archives consommables par Kotlin/Native Android,
ou le runtime libc++ bundlé de K/N rend-il le lien impossible ?

## Ce que faisait l'ancienne lib

`ygdrasil-io/wgpu4k-native` (Kotlin 2.4.10, wgpu-native v29.0.0.0) déclarait
`androidNativeArm64()`/`androidNativeX64()` ; sa CI construisait un APK
`NativeActivity` (`verifyAndroidNativeApks`) contenant `libwgpu4k_demo.so`
pour les deux ABI.

Comme le note le brief, ce contrôle CI ne vérifie que la **présence** des `.so`
dans l'APK : il ne prouve pas que l'application **démarre**. C'est le trou que
ce spike comble, en allant jusqu'au lancement sur émulateur.

## Mesure

### Environnement

| Élément | Valeur |
| --- | --- |
| Hôte | macOS, Apple Silicon (arm64) |
| Émulateur | AVD `Medium_Phone`, `arm64-v8a`, `sys.boot_completed=1` |
| Kotlin/Native | **2.4.10** (prebuilt présent dans `~/.konan`, contrairement à ce que supposait le brief) |
| NDK ayant construit l'archive testée | **27.3.13750724** (r27d), clang 18.0.4 — c'est le NDK présent sur la machine de mesure, sélectionné par `NDK_HOME` via `scripts/build-target.sh` |
| NDK épinglé par la CI | **27.0.11902866** (`.github/workflows/build.yml:86`) — **différent** de celui utilisé ici ; la CI n'a pas été rejouée pour ce spike |
| NDK bundlé par K/N 2.4.10 | `target-toolchain-2-osx-android_ndk`, clang **8.0.7**, `AndroidVersion.txt` = `8.0.7`, binaire construit sous `ndk-release-r20` → ère **NDK r20**, pas r19c |

La différence 27.3 / 27.0 ne change pas la nature de la mesure (le packer ne
produit pas d'objets C++), mais elle n'est pas nulle : le verdict porte sur une
archive construite avec 27.3.

### Reproduction sur émulateur

Deux obstacles d'infrastructure ont dû être levés avant d'obtenir une mesure ;
aucun n'est un problème libc++ :

1. Le clone `--depth 1` du brief omet `--recurse-submodules`. `settings.gradle.kts`
   fait `includeBuild("Kadre")`, et sans le sous-module le build échoue en 9 s :

   ```
   * What went wrong:
   Project with path ':kadre' not found in build ':kadre-build'.
   ```

   Corrigé par `git submodule update --init --depth 1` (`Kadre`, `kextract`).

2. Le toolchain Android bundlé par K/N est un binaire **x86_64** et Rosetta 2
   n'était pas installé. Le lien K/N échoue alors avec :

   ```
   The /Users/chaos/.konan/dependencies/target-toolchain-2-osx-android_ndk/bin/aarch64-linux-android21-clang command returned non-zero exit code: 126.
   output:
   .../bin/aarch64-linux-android21-clang: line 3: .../bin/clang: Bad CPU type in executable
   ```

   Corrigé par `softwareupdate --install-rosetta --agree-to-license`.

Après ces deux correctifs, `./gradlew :demo:android-native:assembleDebug` :

```
> Task :demo:android-native:assembleDebug

BUILD SUCCESSFUL in 53s
71 actionable tasks: 11 executed, 60 up-to-date
```

L'APK contient bien les deux ABI :

```
 13098944  lib/arm64-v8a/libwgpu4k_demo.so
 14115296  lib/x86_64/libwgpu4k_demo.so
```

Installation et lancement (l'identifiant réel est `io.ygdrasil.wgpu.app.native.debug`,
pas `io.ygdrasil.wgpu.app.native` comme dans le brief) :

```
$ adb install -r demo/android-native/build/outputs/apk/debug/android-native-debug.apk
Performing Streamed Install
Success
$ adb shell am start -n io.ygdrasil.wgpu.app.native.debug/android.app.NativeActivity
Starting: Intent { cmp=io.ygdrasil.wgpu.app.native.debug/android.app.NativeActivity }
```

logcat (extrait) :

```
D nativeloader: Load .../base.apk!/lib/arm64-v8a/libwgpu4k_demo.so using class loader ns clns-9 (caller=<unknown>): ok
I Konan_main: ANativeActivity_onCreate called
I NativeActivity: onNativeWindowCreated called
D vulkan  : searching for layers in .../lib/arm64-v8a
I ActivityTaskManager: Displayed io.ygdrasil.wgpu.app.native.debug/android.app.NativeActivity for user 0: +2s944ms
```

Aucune ligne `FATAL`, `UnsatisfiedLink`, `undefined symbol` ou `dlopen failed`.
Le processus reste vivant (`adb shell pidof io.ygdrasil.wgpu.app.native.debug`
→ `3878`), et `adb exec-out screencap` montre le triangle rouge sur fond vert.
**L'application de l'ancienne lib démarre et rend.**

### Reproduction avec l'archive du packer

L'APK ci-dessus lie `wgpu-native` v29, pas notre packer. Pour fermer l'écart,
`libWGPU.a` extrait par l'ancienne lib a été remplacé par
`target/aarch64-linux-android/release/libwgpu_packer.a` (notre archive, wgpu
v30.0.1.0), puis relink forcé.

**Attention** : la tâche d'extraction Gradle
`unzip-libwgpu_native.a-from-wgpu-android-aarch64-release.zip` peut réécrire
`libWGPU.a` depuis le zip téléchargé. Il faut donc :

1. copier notre archive **après** l'extraction ;
2. relancer le build en excluant cette tâche (`-x unzip-...`) ;
3. vérifier le md5 de `libWGPU.a` **après** le build.

Commandes exactes :

```
$ cp target/aarch64-linux-android/release/libwgpu_packer.a \
     /tmp/old-wgpu4k-native/wgpu4k-native/build/native/android-aarch64/lib/libWGPU.a
$ md5 .../libWGPU.a
MD5 (.../libWGPU.a) = 46b70a90f08d77a8e1bc78ee01c50b80   # ← notre archive

$ cd /tmp/old-wgpu4k-native && ./gradlew :demo:android-native:assembleDebug --rerun-tasks \
     -x unzip-libwgpu_native.a-from-wgpu-android-aarch64-release.zip --no-daemon
BUILD SUCCESSFUL in 49s
70 actionable tasks: 70 executed

$ md5 .../libWGPU.a
MD5 (.../libWGPU.a) = 46b70a90f08d77a8e1bc78ee01c50b80   # ← toujours la nôtre après le build
```

Preuve que le `.so` final **contient notre code** (et pas seulement que le lien
a réussi) :

```
$ llvm-nm -gU demo/common/build/bin/androidNativeArm64/debugShared/libwgpu4k_demo.so | grep -c wgpu_packer
37
$ strings -a .../libwgpu4k_demo.so | grep "panique contenue"
$[wgpu_packer] panique contenue dans
```

(la chaîne vient de `ffi/src/ffi_utils.rs:13` ; les 37 symboles incluent
`_ZN11wgpu_packer...`, `...wgpu_packer..objects..WGPUDeviceImpl..drop...`, etc.)

Notre archive exporte 228 symboles `wgpu*` contre 226 pour l'ancienne, et c'est
un **sur-ensemble** : aucun symbole exigé par le cinterop v29 ne manque.

Install + lancement :

```
$ adb install -r demo/android-native/build/outputs/apk/debug/android-native-debug.apk
Success
$ adb shell am start -n io.ygdrasil.wgpu.app.native.debug/android.app.NativeActivity
Starting: Intent { cmp=io.ygdrasil.wgpu.app.native.debug/android.app.NativeActivity }
```

logcat (extrait) :

```
D nativeloader: Load .../base.apk!/lib/arm64-v8a/libwgpu4k_demo.so using class loader ns clns-9 (caller=<unknown>): ok
I Konan_main: ANativeActivity_onCreate called
I NativeActivity: onNativeWindowCreated called
I Konan_main: DEBUG : Instance::new: created Vulkan backend
I Konan_main: TRACE : Device::create_render_pipeline -> RenderPipelineId(0,1)
I Konan_main: TRACE : RenderPass::draw 3 1 0 0
I Konan_main: TRACE : Finished encoding render pass (success)
I Konan_main: TRACE : Queue::submit returned submit index 1
I ActivityTaskManager: Displayed io.ygdrasil.wgpu.app.native.debug/android.app.NativeActivity for user 0: +4s2ms
```

Aucune ligne `FATAL`, `tombstone`, `SIGSEGV`, `SIGABRT`, `UnsatisfiedLink`,
`undefined symbol` ou `dlopen failed`. Processus vivant
(`adb shell pidof io.ygdrasil.wgpu.app.native.debug` → `4810`). Capture :
`/tmp/packer-android-native-v2.png` — triangle rouge sur fond vert.

**Note méthodologique** : les lignes `TRACE :` sont émises par le Kotlin du
démo (`demo/common/src/commonMain/kotlin/ext.kt`, `println("TRACE : ...")`) à
partir de messages de `wgpu_core`, présent dans **les deux** archives (v29 comme
v30). Elles ne distinguent donc **pas** v29 de notre v30. La preuve que
l'archive liée est la nôtre est le couple **md5 de `libWGPU.a`** + **symboles
`wgpu_packer` et chaîne `[wgpu_packer] panique contenue` dans le `.so` final**.

### Symboles libc++ non résolus

Le motif `find` du brief (`-path "*android_arm64*"`) ne trouve rien : le
libc++ bundlé par K/N vit sous `~/.konan/dependencies/target-toolchain-2-osx-android_ndk/`
(le nom du dossier est `android_ndk`, pas `android_arm64`). Chemin réel :

```
/Users/chaos/.konan/dependencies/target-toolchain-2-osx-android_ndk/sysroot/usr/lib/aarch64-linux-android/libc++_static.a
```

Comparaison des symboles non résolus (`llvm-nm -u`), filtrés sur
`std::__ndk1|__libcpp|__cxx_atomic` :

```
=== /tmp/libWGPU.a.orig   (ancienne lib, wgpu-native v29, android-aarch64) ===
  -> 0 symboles
=== target/aarch64-linux-android/release/libwgpu_packer.a   (ce packer, wgpu v30) ===
  -> 0 symboles
=== libc++ K/N définit ? ===
0
```

**Portée de ce filtre** : il ne couvre que les symboles manglés sous
`std::__ndk1` (le namespace inline de libc++ NDK), `__libcpp*` et
`__cxx_atomic*`. Il ne couvre **pas** `__cxa_*` ni `_Unwind_*`, et « 0
symboles » ne signifie donc **pas** « aucune référence au runtime C++ ».

Mesures complémentaires sur les mêmes archives :

| Filtre | Ancienne (v29) | Nôtre (v30) |
| --- | --- | --- |
| `std::__ndk1` / `__libcpp` / `__cxx_atomic` | 0 | 0 |
| `__cxa_*` | 0 | 0 |
| `_Unwind_*` | **124** | **127** |

Les `_Unwind_*` existent donc bien, dans **les deux** archives. Ce ne sont pas
des symboles libc++ : ils sont résolus au lien final par le runtime de
déroulement fourni par le toolchain K/N. Vérification : le `.so` final ne les
laisse pas indéfinis (`llvm-nm -D -u` → 0 `_Unwind_*`), et l'application
démarre. Le libc++ de K/N 2.4.10 ne définit pas `__libcpp_verbose_abort` ni
`__libcpp_atomic_wait` (compte = 0), mais rien dans les archives ne les demande.

Même constat pour `androidNativeX64` (0 symbole sous le filtre libc++). Le
`.so` final ne porte lui non plus aucune référence `std::__ndk1`/`__libcpp`/
`__cxx_atomic` non résolue (`llvm-nm -D -u` → 0).

## Verdict

VERDICT: supported

Une application Kotlin/Native 2.4.10 liant l'archive de ce packer se construit,
s'installe et **s'exécute** sur un émulateur Android arm64 : le triangle est
rendu via Vulkan, le processus ne meurt pas, et le `.so` final contient
effectivement du code du packer (37 symboles `wgpu_packer`, chaîne
`[wgpu_packer] panique contenue`) — l'archive liée est donc bien la nôtre, et
non celle de wgpu-native v29.

L'affirmation du spike dawn-packer (« le lien K/N laisse des symboles libc++
non résolus, l'app crashe au chargement ») ne se reproduit pas ici. Le spike
dawn-packer l'avait rapporté pour **son** archive C++ (Dawn référence
`__libcpp_verbose_abort` et consorts) ; ce spike-ci n'a pas testé Dawn et ne
peut donc pas confirmer sa cause. Notre packer est du Rust et n'émet aucune
référence libc++.

## Conséquence pour le packer

Les deux cibles passent en `v1` avec `linkages: ["static"]` dans
`targets/matrix.json` :

- `androidNativeArm64` — `aarch64-linux-android`, `arm64-v8a`
- `androidNativeX64` — `x86_64-linux-android`, `x86_64`

`linkLibs: "-llog -landroid"`, `deployEnv: {"ANDROID_API": "24"}`,
`backends: ["vulkan", "gles", "null"]`, `runner: "ubuntu-24.04"`.

### Limites de ce qui a été prouvé

- **arm64 uniquement, sur émulateur.** `androidNativeX64` n'a été validé que
  par la comparaison de symboles (0 référence libc++ non résolue) et par le
  fait que son `.so` est bien empaqueté dans l'APK — pas par un lancement sur
  un émulateur x86_64.
- **Pas de test sur appareil physique.** L'émulateur utilise le Vulkan de
  l'hôte ; un GPU mobile réel peut se comporter autrement (pilotes, extensions),
  mais cela ne relève pas de libc++.
- **Le binaire de test est celui de l'ancienne lib** (cinterop wgpu v29) dans
  lequel on a substitué notre archive. C'est une preuve de liaison et
  d'exécution de notre archive, pas un test d'une intégration consommateur
  écrite pour wgpu v30.
- **`linkages: ["static"]` seulement.** Le mode `shared` n'a pas été testé ;
  l'ancienne lib déclarait les deux, mais rien ici ne prouve le `.so`.
- **NDK 27.3.13750724 ici, 27.0.11902866 en CI.** L'archive mesurée a été
  construite avec le NDK local 27.3 ; la CI épingle 27.0. La CI n'a pas été
  rejouée pour ce spike.
- **`androidNativeX64` a nécessité un contournement local de bindgen.**
  `ffi/build.rs` n'a pas de branche Android ; la compilation x86_64 a été
  obtenue en passant
  `BINDGEN_EXTRA_CLANG_ARGS_<triple>="--target=... --sysroot=<NDK sysroot>"`.
  Si la CI échoue sur cette cible, `ffi/build.rs` aura besoin d'une branche
  Android comme ses branches iOS/macOS.
- Le spike dawn-packer reste valable **pour Dawn** : son archive C++ référence
  bien des symboles absents du libc++ de K/N. Ce verdict ne l'infirme pas ; il
  montre que le problème est propre au C++ de Dawn, pas au mécanisme de lien
  K/N Android.
