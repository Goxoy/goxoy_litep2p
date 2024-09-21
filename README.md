# Goxoy LiteP2P
Node'ların bağlantı ve mesaj iletim sürecinin yöneten kitaplık.
Not : 
* Bu kitaplık TCP bağlantısı üzerinden iletişim kuruyor.
* İletişim hattı her mesaj sonrası kapatılıyor.
* Bu kitaplık node sayılarını ve adreslerini yönetmek üzere tasarlanmıştır.

## Config dosyası
```json
{
    "debug": false,
    "store_node_list": true,
    "addr": "127.0.0.1:1111",
    "bootstrap": [
        "127.0.0.1:1111"
    ]
}
```

## Kullanım / Örnekler

```rust
// önce nesneyi oluşturup, sonrasında ayarları tanımlayabilirsiniz.
let mut msg_pool = MessagePool::new();

// config dosyasının adını parametre olarak verin
msg_pool.start(config_file_name);

// gerçekleşen Event için geri bir işlem dönecek
loop{
    match msg_pool.on_event() {
        EventType::OnMessage(_income_msg) => {
            // diğer node veya node'lardan mesaj gönderildiğinde
            // bu bölüm devreye giriyor
        }
        EventType::OnWait() => {
            // eğer hiç bir işlem yok ise
            // bu bölüm devreye giriyor
        }
        EventType::OnNodesSynced(_node_list_hash) => {
            // bağlanan node'lar sayı ve durum açısından senkron olduğunda
            // bu bölüm devreye giriyor
        }
        EventType::OnNodeStatusChanged(node_addr, node_status) => {
            // eğer bir node Online veya Offline durumuna geçerse
            // bu bölüm devreye giriyor
        },
    }
}
```

  
## Lisans

[MIT](https://choosealicense.com/licenses/mit/)