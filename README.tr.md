# Hien

Rust ve egui ile yazılmış, hızlı bir Windows not defteri. Notlar `%APPDATA%\Hien\notes` klasöründe düz `.txt` dosyaları olarak durur, yazmayı bıraktıktan 400 ms sonra kendiliğinden kaydedilir.

[İndir](https://github.com/misutesu-desu/hien/releases/latest) · [Site](https://misutesu-desu.github.io/hien/) · [English](README.md)

## Özellikler

- Notlar arasında anında arama, ilk satır notun adı olur.
- Bul ve değiştir: tek tek ya da hepsini birden. Büyük küçük harf ve tam kelime seçenekleri var, Türkçe I/ı ve İ/i aramada eşleşir.
- Kelime ekleme: aranan kelimenin önüne ya da arkasına başka bir kelime yazar. `kedi` için `tekir` seçilirse her `kedi`, `tekir kedi` olur. Kelime zaten oradaysa tekrar eklenmez, araya boşluk konup konmayacağı seçilebilir.
- Toplu işlemler tek düğmeyle geri alınır.
- Silinen not `trash` klasörüne taşınır, durum çubuğundan geri getirilebilir.
- İngilizce ve Türkçe arayüz. Sistem diline göre seçilir, Ayarlar menüsünden değişir.
- `.txt` dosyalarını açar. Hien `.txt` için varsayılan uygulama yapılırsa çift tıklanan dosya Hien'de açılır. Dosya satır sonları korunarak yerine kaydedilir, Dosyayı kapat ona dokunmadan listeden çıkarır. Dosyalar UTF-8 olmalı. Hien zaten açıksa dosya o pencerede açılır.
- Pencere kapatılınca Hien arka planda çalışmaya devam eder, sonraki dosya hemen açılır. Ayarlar menüsünde Hien'den çık (Ctrl+Q), bunu kapatma seçeneği ve oturum açılınca Hien'i gizli başlatan Windows ile başlat var.
- Koyu, açık ve sistem teması.

## Kısayollar

| Tuş | İş |
|---|---|
| Ctrl+N | Yeni not |
| Ctrl+F | Bul |
| Ctrl+H | Değiştir |
| Ctrl+E | Kelime ekle |
| F3, Shift+F3 | Sonraki, önceki eşleşme |
| Enter | Panelde o satırın işlemini yapar |
| Ctrl+K | Notlarda ara |
| Alt+Yukarı, Alt+Aşağı | Önceki, sonraki not |
| Ctrl+S | Hemen kaydet |
| Ctrl+Q | Hien'den çık |
| Orta tık | Fareyi oynatarak kaydırır, tekrar tıklayınca durur. Tekerlek basılı tutulursa yalnız basılıyken kaydırır. |

Bul paneli açılırken editörde seçili metin varsa arama kutusuna gelir.

## Derleme

```bash
cargo build --release
```

Çıktı `target/release/hien.exe`. Testler için `cargo test`.
