use crate::models::WinetricksVerb;

/// Curated subset of winetricks verbs that cover the vast majority of Windows
/// game dependencies. Running arbitrary verbs is intentionally not exposed.
pub fn verbs() -> Vec<WinetricksVerb> {
    let entry = |verb: &str, label: &str, description: &str, category: &str| WinetricksVerb {
        verb: verb.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        category: category.to_string(),
    };

    vec![
        entry(
            "vcrun2022",
            "Visual C++ 2015-2022",
            "Required by most modern games, launchers and anti-cheat installers.",
            "Runtimes",
        ),
        entry(
            "vcrun2019",
            "Visual C++ 2015-2019",
            "Older CRT runtime used by many DirectX 11 titles.",
            "Runtimes",
        ),
        entry(
            "vcrun2010",
            "Visual C++ 2010",
            "Legacy runtime for DX9-era games.",
            "Runtimes",
        ),
        entry(
            "dotnet48",
            ".NET Framework 4.8",
            "Needed by many launchers, tools and smaller indie games.",
            "Runtimes",
        ),
        entry(
            "dotnet6",
            ".NET 6 desktop runtime",
            "Modern .NET games and tools.",
            "Runtimes",
        ),
        entry(
            "d3dx9",
            "DirectX 9 (d3dx9)",
            "DirectX 9 redistributable helper libraries.",
            "DirectX",
        ),
        entry(
            "d3dx11_43",
            "DirectX 11 (d3dx11_43)",
            "D3DX11 helpers used by older engines.",
            "DirectX",
        ),
        entry(
            "dxvk",
            "DXVK (DirectX 9-11)",
            "Installs DXVK into this container. Required by the DXVK backend when the Wine build does not bundle it.",
            "Graphics",
        ),
        entry(
            "xact",
            "XACT audio",
            "Audio engine used by many Unreal Engine 3 titles.",
            "Audio",
        ),
        entry(
            "faudio",
            "FAudio / XAudio2",
            "Modern XAudio2 implementation with lower audio latency.",
            "Audio",
        ),
        entry(
            "mfinst",
            "Media Foundation",
            "Video playback for in-game cinematics and cutscenes.",
            "Media",
        ),
        entry(
            "corefonts",
            "Core fonts",
            "Arial, Times New Roman, Courier and other Windows fonts.",
            "Fonts",
        ),
        entry(
            "vulkan-rt",
            "Vulkan runtime",
            "Vulkan loader used by the DXVK / MoltenVK path.",
            "Graphics",
        ),
    ]
}

pub fn is_known(verb: &str) -> bool {
    verbs().iter().any(|entry| entry.verb == verb)
}
