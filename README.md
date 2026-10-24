# Hien

Rust ve egui ile yazılmış, hızlı açılan bir not defteri. Notlar `%APPDATA%\Hien\notlar` klasöründe düz `.txt` dosyaları olarak durur, yazmayı bıraktıktan 400 ms sonra kendiliğinden kaydedilir.

## Özellikler

- Notlar arasında anında arama, ilk satır notun adı olur.
- Bul ve değiştir: tek tek ya da hepsini birden. Büyük küçük harf ve tam kelime seçenekleri var, Türkçe I/ı ve İ/i aramada eşleşir.
- Kelime ekleme: aranan kelimenin önüne ya da arkasına başka bir kelime yazar. Örneğin `kedi` için `tekir` seçilirse her `kedi`, `tekir kedi` olur. Kelime zaten oradaysa tekrar eklenmez, araya boşluk konup konmayacağı seçilebilir.
- Toplu işlemler tek düğmeyle geri alınır.
- Silinen not `cop` klasörüne taşınır, durum çubuğundan geri getirilebilir.
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
| Esc | Paneli kapat |

Bul paneli açılırken editörde seçili metin varsa arama kutusuna gelir.

## Derleme

```bash
cargo build --release
```

Çıktı `target/release/hien.exe`. Testler için `cargo test`.
