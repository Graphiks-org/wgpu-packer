plugins { kotlin("multiplatform") version "2.1.20" }

val wgpuDist = providers.gradleProperty("wgpuDist").get() // racine de l'archive décompressée

kotlin {
    macosArm64("native") {
        binaries {
            executable {
                entryPoint = "main"
                linkerOpts(
                    "-framework", "Metal", "-framework", "QuartzCore",
                    "-framework", "CoreVideo", "-framework", "IOKit",
                    "-framework", "Foundation",
                    "-L$wgpuDist/lib", "-lwgpu_packer",
                )
            }
        }
        compilations.getByName("main") {
            cinterops.create("wgpu") {
                defFile(file("../wgpu.def"))
                includeDirs("$wgpuDist/include")
                extraOpts("-libraryPath", "$wgpuDist/lib")
            }
        }
    }
}
