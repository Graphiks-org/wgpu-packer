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
| NDK du packer | 27.3.13750724 (r27d) |
| NDK bundlé par K/N 2.4.10 | `target-toolchain-2-osx-android_ndk`, clang **8.0.7**, `AndroidVersion.txt` = `8.0.7`, binaire construit sous `ndk-release-r20` → ère **NDK r20**, pas r19c |

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
Le processus reste vivant (`adb shell pidof` → PID), et `adb exec-out screencap`
montre le triangle rouge sur fond vert. **L'application démarre et rend.**

#### Reproduction avec l'archive du packer

L'APK ci-dessus lie `wgpu-native` v29, pas notre packer. Pour fermer l'écart,
`libWGPU.a` extrait par l'ancienne lib a été remplacé par
`target/aarch64-linux-android/release/libwgpu_packer.a` (notre archive, wgpu
v30.0.1.0), puis relink forcé (`--rerun-tasks`). Notre archive exporte 228
symboles `wgpu*` contre 226 pour l'ancienne, et c'est un **sur-ensemble** :
aucun symbole exigé par le cinterop v29 ne manque.

`BUILD SUCCESSFUL in 1m 14s`, puis install + lancement. logcat :

```
I Konan_main: TRACE : Device::create_render_pipeline -> Id(0,1)
I Konan_main: TRACE : RenderPass::draw 3 1 0 0
I Konan_main: TRACE : Finished encoding render pass (success)
I Konan_main: TRACE : Queue::submit returned submit index 1
I ActivityTaskManager: Displayed io.ygdrasil.wgpu.app.native.debug/android.app.NativeActivity for user 0: +2s130ms
```

Ces `TRACE` sont émis par **notre** code Rust (le packer), pas par
wgpu-native : c'est la preuve que l'archive liée est bien la nôtre. Le
processus survit, aucune erreur de chargement, et le triangle s'affiche
(`/tmp/packer-android-native.png`).

### Symboles libc++ non résolus

Le motif `find` du brief (`-path "*android_arm64*"`) ne trouve rien : le
libc++ bundlé par K/N vit sous `~/.konan/dependencies/target-toolchain-2-osx-android_ndk/`
(le nom du dossier est `android_ndk`, pas `android_arm64`). Chemin réel :

```
/Users/chaos/.konan/dependencies/target-toolchain-2-osx-android_ndk/sysroot/usr/lib/aarch64-linux-android/libc++_static.a
```

Comparaison des symboles non résolus (`llvm-nm -u`, filtrés sur
`std::__ndk1|__libcpp|__cxx_atomic`) :

```
=== /tmp/libWGPU.a.orig   (ancienne lib, wgpu-native v29, android-aarch64) ===
  -> 0 symboles
=== target/aarch64-linux-android/release/libwgpu_packer.a   (ce packer, wgpu v30) ===
  -> 0 symboles
=== libc++ K/N définit ? ===
0
```

Autrement dit : les deux archives ne référencent **aucun** symbole libc++ non
résolu. Le libc++ de K/N 2.4.10 ne définit effectivement pas
`__libcpp_verbose_abort` ni `__libcpp_atomic_wait` (compte = 0), mais rien dans
les archives ne les demande. Même constat pour `androidNativeX64` (0 symbole).
Le `.so` final ne porte lui non plus aucune référence libc++ non résolue
(`llvm-nm -D -u` → 0).

## Verdict

VERDICT: supported

Une application Kotlin/Native 2.4.10 liant l'archive de ce packer se construit,
s'installe et s'exécute sur un émulateur Android arm64 : le triangle est rendu
via Vulkan et le processus ne meurt pas. L'affirmation du spike dawn-packer
(« le lien K/N laisse des symboles libc++ non résolus, l'app crashe au
chargement ») ne se reproduit pas ici, parce qu'elle concernait l'archive C++ de
**Dawn**, qui référence `__libcpp_verbose_abort` et consorts ; notre packer est
du Rust et n'émet aucune référence C++ runtime.

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
- Le spike dawn-packer reste valable **pour Dawn** : son archive C++ référence
  bien des symboles absents du libc++ de K/N. Ce verdict ne l'infirme pas ; il
  montre que le problème est propre au C++ de Dawn, pas au mécanisme de lien
  K/N Android.
