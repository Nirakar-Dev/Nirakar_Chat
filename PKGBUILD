pkgname=nirakar-chat
pkgver=0.1.0
pkgrel=1
pkgdesc="Nirakar Chat - Ethereal P2P Messaging"
arch=('x86_64')
url="https://github.com/nirakar/chat"
license=('MIT')
depends=('webkit2gtk-4.1' 'curl' 'wget' 'openssl')
makedepends=('cargo' 'npm')
source=("local://\$pkgname-\$pkgver.tar.gz")
sha256sums=('SKIP')

build() {
  cd "$srcdir/"
  npm install
  npm run tauri build
}

package() {
  cd "$srcdir/src-tauri/target/release/bundle/deb"
  # In a real PKGBUILD, we would extract the deb or manually place binaries.
  # Since Tauri builds .deb and AppImage, we can just install the built binary.
  install -Dm755 "../../nirakar-chat" "$pkgdir/usr/bin/nirakar-chat"
}
