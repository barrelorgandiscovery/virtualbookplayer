
# notes for android build

For the moment midi output needs a rewrite for ndk

export ANDROID_HOME=~/Android/sdk
export NDK_HOME=$ANDROID_HOME/ndk

rustup target add aarch64-linux-android
cargo install cargo-ndk

cargo ndk -t arm64-v8a -o app/src/main/jniLibs/  build

./gradlew build
./gradlew installDebug
adb shell am start -n co.realfit.agdkeframe/.MainActivity

