# gomobile calls these classes from native code by name.
-keep class go.** { *; }
-keep class app.lumen.core.** { *; }
# Tauri finds plugin commands and argument classes by reflection.
-keep class app.lumen.plugin.** { *; }
