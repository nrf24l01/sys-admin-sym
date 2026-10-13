# Создайте VLAN управления

**VLAN** разделяет коммутатор на разные широковещательные домены L2. Кадры VLAN 10
не переходят в VLAN 20 обычным переключением. Это отделяет управление от клиентов.

**Access-порт** принадлежит одной VLAN и обычно передаёт серверу кадры без тегов.
**Trunk-порт** переносит несколько VLAN с тегами. Для управления в этом примере
используются access-порты и нетегированные серверные интерфейсы.

## В консоли коммутатора

Убедитесь, что порты 1 и 2 подключены к интерфейсам управления:

```text
enable
configure terminal
vlan 10
name Management
exit
interface Gi1/0/1
switchport mode access
switchport access vlan 10
no shutdown
exit
interface Gi1/0/2
switchport mode access
switchport access vlan 10
no shutdown
exit
end
show vlan brief
show interfaces status
write memory
```

`vlan 10` создаёт VLAN; `switchport access vlan 10` назначает её порту;
`no shutdown` включает интерфейс; `write memory` сохраняет настройки для reload.
Настройка через GUI удовлетворяет тем же условиям.

Оба порта должны принадлежать одной созданной VLAN, отличной от стандартной VLAN 1.
Число 10 — пример: руководство принимает и вашу VLAN. Номер VLAN не является
IP-адресом или маской подсети.

Далее: [назначьте частные IP-адреса](management-addresses.md).
Подробнее: [L2 и VLAN](../reference/layer2-vlans.md).
