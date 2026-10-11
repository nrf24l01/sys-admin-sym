import { computed, ref, watch } from 'vue'

const en = {
  skip: 'Skip to content', gameplay: 'The game', terminal: 'game-ssh', downloads: 'Downloads', docs: 'Documentation', source: 'Source code',
  eyebrow: 'A hands-on infrastructure simulator',
  heroLines: ['Build the rack.', 'Configure the network.', 'Run the provider.'],
  intro: 'From the first patch cable to your own hosting business. Build the hardware, make the connections, and find out what it takes to keep everything running.',
  downloadGame: 'Download the game', includes: 'Game + game-ssh included', platforms: 'Windows & Linux · x86-64',
  seeGameplay: 'Explore the game', rackLabel: 'Equipment from the game', rackCaption: 'Servers, switches, routers, and power. Every part has a purpose.',
  rackFront: 'Front', rackRear: 'Rear', rackFace: 'Rack equipment view',
  rackNote: 'Start with an empty rack. Make every connection count.',
  strip: ['Physical hardware', 'Linux & IOS-style consoles', 'Networking & hosting'],
  sectionLabel: '01 / Inside the simulation', sectionTitle: 'It starts with a cable.\nIt ends with a business.',
  sectionIntro: 'The physical details matter. So do the commands you type. Build an infrastructure where the two work together.',
  buildTitle: 'Build it from the ground up.',
  buildText: 'Choose your server chassis. Install CPU, memory, drives, and network cards. Mount switches, routers, and power equipment. An empty rack is only the beginning.',
  buildDetail: 'Hardware compatibility · Rack placement · Power budgets',
  networkTitle: 'Make the network work.',
  networkText: 'Cut copper leads, route fiber, and connect the right ports. Configure VLANs, gateways, and routing. When a link goes down, follow the problem back to its source.',
  networkDetail: 'Physical links · VLANs · IPv4 routing',
  hostingTitle: 'Become the provider.',
  hostingText: 'Prepare virtualization nodes, publish hosting products, and provision customer services. Balance capacity and demand. Your network and hardware determine what you can deliver.',
  hostingDetail: 'Virtual machines · Resource contention · Customer subscriptions',
  actualGameplay: 'Actual gameplay', shopCaption: 'Select hardware. Install the parts that fit.', cablingCaption: 'Rear rack view. Power and network routes are physical.',
  inspectorCaption: 'A server is more than a box. Inspect hardware, power, and ports.',
  sshLabel: '02 / Beyond the game window', sshTitle: 'Your terminal.\nYour machines.',
  sshText: 'Use game-ssh to connect your own terminal to device consoles in the running game. Configure a server, bring up a switch, or run a command from a script. Changes appear in the game.',
  sshFeatures: ['Tab completion', 'Persistent history', 'Scriptable commands'], sshDownload: 'Get the standalone client',
  sshNote: 'Included with the game. The standalone client needs a running game; it uses the game’s console bridge, not the SSH protocol.',
  copy: 'Copy commands', copied: 'Copied', copyFailed: 'Select and copy the commands manually.',
  downloadLabel: '03 / Get started', downloadTitle: 'Your next project\nstarts here.', downloadText: 'Pick your platform. Get the game, or just the terminal client.',
  unsupported: 'You’re browsing on a platform without a native build. Choose a Windows or Linux download below.',
  package: 'Package', release: 'Release', file: 'File', download: 'Download',
  gameName: 'Cloud Provider Simulator', gameDescription: 'The full game, runtime assets, and game-ssh.',
  clientName: 'game-ssh', clientDescription: 'The standalone terminal client. Game required.',
  development: 'Not available', latestBuild: 'Latest uploaded build', zip: 'ZIP archive', directDownload: 'Download ZIP',
  checking: 'Checking published releases…', noReleases: 'Published ZIPs are not available yet. Downloads will appear here when a release is published.',
  releaseError: 'Couldn’t check published releases. Try again to load the download files.',
  retry: 'Try again',
  checksum: 'SHA-256', notes: 'Release notes', configured: 'Hosted package',
  installation: 'Before your first boot', windowsInstall: 'Windows installation', linuxInstall: 'Linux installation', consoleInstall: 'Connecting game-ssh',
  windowsHelp: 'Extract the downloaded package. Keep cloud-provider-sim.exe and the assets folder together, then launch the executable. A working graphics driver is required. Rust and the Visual C++ redistributable are not needed.',
  linuxHelp: 'Extract the package and keep the executable beside its assets folder. Use a graphical desktop with working graphics drivers, glibc, and X11 libraries. Make the executable runnable, then start it:',
  consoleHelp: 'Keep the game running. Open Settings and enable/configure the remote console. Run game-ssh --list, then select a powered-on device by ID. Enter the password configured in the game.',
  docsPrompt: 'Need a starting point?', networkGuide: 'Network setup', linuxGuide: 'Linux guest guide', terminalGuide: 'game-ssh instructions',
  footerLine: 'Build something that works.', footerNote: 'An independent infrastructure simulation project.', rights: 'Equipment names and trademarks belong to their respective owners.',
}

const ru: typeof en = {
  skip: 'Перейти к содержимому', gameplay: 'Об игре', terminal: 'game-ssh', downloads: 'Скачать', docs: 'Документация', source: 'Исходный код',
  eyebrow: 'Симулятор инфраструктуры с вниманием к деталям',
  heroLines: ['Соберите стойку.', 'Настройте сеть.', 'Станьте провайдером.'],
  intro: 'От первого патч-корда до собственного хостинга. Соберите оборудование, соедините устройства и узнайте, что нужно для бесперебойной работы.',
  downloadGame: 'Скачать игру', includes: 'Игра + game-ssh в комплекте', platforms: 'Windows и Linux · x86-64',
  seeGameplay: 'Посмотреть игру', rackLabel: 'Оборудование из игры', rackCaption: 'Серверы, коммутаторы, маршрутизаторы и питание. У каждой детали есть задача.',
  rackFront: 'Спереди', rackRear: 'Сзади', rackFace: 'Вид оборудования в стойке', rackNote: 'Начните с пустой стойки. Каждое соединение имеет значение.',
  strip: ['Физическое оборудование', 'Консоли Linux и в стиле IOS', 'Сети и хостинг'],
  sectionLabel: '01 / Внутри симуляции', sectionTitle: 'Всё начинается с кабеля.\nИ становится бизнесом.',
  sectionIntro: 'Физические детали важны. Как и введённые команды. Постройте инфраструктуру, в которой всё работает вместе.',
  buildTitle: 'Соберите всё с нуля.',
  buildText: 'Выберите корпус сервера. Установите процессор, память, диски и сетевые карты. Смонтируйте коммутаторы, маршрутизаторы и питание. Пустая стойка — лишь начало.',
  buildDetail: 'Совместимость компонентов · Монтаж в стойке · Бюджет питания',
  networkTitle: 'Заставьте сеть работать.',
  networkText: 'Нарежьте медные кабели, проложите оптику и соедините нужные порты. Настройте VLAN, шлюзы и маршруты. Если связь пропала, найдите причину в реальной цепочке соединений.',
  networkDetail: 'Физические соединения · VLAN · Маршрутизация IPv4',
  hostingTitle: 'Станьте провайдером.',
  hostingText: 'Подготовьте узлы виртуализации, создайте тарифы и предоставляйте услуги клиентам. Учитывайте ресурсы и спрос. Возможности хостинга определяются вашей сетью и оборудованием.',
  hostingDetail: 'Виртуальные машины · Конкуренция за ресурсы · Подписки клиентов',
  actualGameplay: 'Кадр из игры', shopCaption: 'Выберите оборудование и совместимые компоненты.', cablingCaption: 'Стойка сзади. Кабели питания и сети имеют физические маршруты.',
  inspectorCaption: 'Сервер — это не просто корпус. Проверяйте компоненты, питание и порты.',
  sshLabel: '02 / За пределами игрового окна', sshTitle: 'Ваш терминал.\nВаши машины.',
  sshText: 'Подключите свой терминал к консолям устройств работающей игры через game-ssh. Настройте сервер или коммутатор, выполните команды из скрипта. Изменения появятся в игре.',
  sshFeatures: ['Дополнение по Tab', 'История команд', 'Выполнение скриптов'], sshDownload: 'Скачать отдельный клиент',
  sshNote: 'Входит в комплект игры. Отдельному клиенту нужна запущенная игра; он использует игровой консольный мост, а не протокол SSH.',
  copy: 'Скопировать команды', copied: 'Скопировано', copyFailed: 'Выделите и скопируйте команды вручную.',
  downloadLabel: '03 / Начните здесь', downloadTitle: 'Ваш следующий проект\nначинается здесь.', downloadText: 'Выберите платформу. Скачайте игру или только консольный клиент.',
  unsupported: 'Для вашей платформы нет нативной сборки. Ниже можно выбрать Windows или Linux.',
  package: 'Пакет', release: 'Версия', file: 'Файл', download: 'Скачать',
  gameName: 'Cloud Provider Simulator', gameDescription: 'Игра, необходимые ресурсы и game-ssh.',
  clientName: 'game-ssh', clientDescription: 'Отдельный консольный клиент. Требуется игра.',
  development: 'Недоступно', latestBuild: 'Последняя загруженная сборка', zip: 'ZIP-архив', directDownload: 'Скачать ZIP',
  checking: 'Проверяем опубликованные версии…', noReleases: 'Опубликованных ZIP пока нет. Файлы появятся здесь после публикации версии.',
  releaseError: 'Не удалось проверить версии. Повторите попытку, чтобы загрузить список файлов.',
  retry: 'Повторить',
  checksum: 'SHA-256', notes: 'Описание версии', configured: 'Размещённый пакет',
  installation: 'Перед первым запуском', windowsInstall: 'Установка в Windows', linuxInstall: 'Установка в Linux', consoleInstall: 'Подключение game-ssh',
  windowsHelp: 'Распакуйте пакет. Оставьте cloud-provider-sim.exe и папку assets вместе, затем запустите программу. Нужен рабочий графический драйвер. Rust и Visual C++ Redistributable не требуются.',
  linuxHelp: 'Распакуйте пакет и оставьте программу рядом с папкой assets. Нужны графическая среда, рабочий драйвер, glibc и библиотеки X11. Разрешите запуск файла и запустите игру:',
  consoleHelp: 'Оставьте игру запущенной. В Settings включите и настройте удалённую консоль. Выполните game-ssh --list и выберите включённое устройство по ID. Введите пароль, заданный в игре.',
  docsPrompt: 'С чего начать?', networkGuide: 'Настройка сети', linuxGuide: 'Руководство по Linux', terminalGuide: 'Инструкция game-ssh',
  footerLine: 'Постройте то, что работает.', footerNote: 'Независимый проект симуляции инфраструктуры.', rights: 'Названия оборудования и товарные знаки принадлежат их владельцам.',
}

function savedLocale(): 'en' | 'ru' {
  try { return localStorage.getItem('cloud-provider-language') === 'ru' ? 'ru' : 'en' }
  catch { return 'en' }
}
const locale = ref<'en' | 'ru'>(savedLocale())
watch(locale, value => {
  document.documentElement.lang = value
  document.querySelector('meta[name="description"]')?.setAttribute('content', value === 'ru' ? 'Соберите стойку, настройте сеть и станьте провайдером. Скачайте Cloud Provider Simulator и game-ssh для Windows и Linux. Руководства на русском.' : 'Build the rack. Configure the network. Run the provider. Download Cloud Provider Simulator and game-ssh for Windows and Linux. Read the game guides here.')
  try { localStorage.setItem('cloud-provider-language', value) } catch { /* Privacy mode keeps the preference in memory. */ }
}, { immediate: true })

export function setLocale(value: 'en' | 'ru') { locale.value = value }

export function useLocale() {
  return { locale, t: computed(() => locale.value === 'ru' ? ru : en), setLocale, toggle: () => { locale.value = locale.value === 'en' ? 'ru' : 'en' } }
}
