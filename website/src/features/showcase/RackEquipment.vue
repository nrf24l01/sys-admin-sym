<script setup lang="ts">
import { computed, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { useLocale } from '@/shared/locale'

const { t, locale } = useLocale()
const rear = ref(false)
type Face = { image: string; uv: [number, number, number, number] }
const equipment: { name: string; unit: number; front: Face; back: Face; double?: boolean }[] = [
  { name: 'Cisco ISR C1111', unit: 42, front: { image: 'router-front.png', uv: [0, 193 / 683, 1, 480 / 683] }, back: { image: 'router-rear.png', uv: [0, 0.31, 1, 0.70] } },
  { name: 'Catalyst C1000', unit: 40, front: { image: 'switch-front.png', uv: [0, 210 / 666, 1, 434 / 666] }, back: { image: 'switch-rear.png', uv: [0.002, 0.45, 0.997, 0.755] } },
  { name: 'PowerEdge R360 / 01', unit: 37, front: { image: 'server-front.png', uv: [0.009, 0.48, 0.995, 0.84] }, back: { image: 'server-rear.png', uv: [0, 0, 1, 1] } },
  { name: 'PowerEdge R360 / 02', unit: 36, front: { image: 'server-front.png', uv: [0.009, 0.48, 0.995, 0.84] }, back: { image: 'server-rear.png', uv: [0, 0, 1, 1] } },
  { name: 'Smart-UPS 1500', unit: 33, double: true, front: { image: 'ups.png', uv: [90 / 2048, 0, 1958 / 2048, 0.5] }, back: { image: 'ups.png', uv: [0, 0.5, 1, 1] } },
]
const rows = computed(() => equipment.map(item => ({ ...item, face: rear.value ? item.back : item.front })))
function crop(uv: Face['uv']) {
  const [left, top, right, bottom] = uv
  return { width: `${100 / (right - left)}%`, height: `${100 / (bottom - top)}%`, left: `${-100 * left / (right - left)}%`, top: `${-100 * top / (bottom - top)}%` }
}
</script>

<template>
  <figure class="min-w-0">
    <div class="relative overflow-hidden bg-equipment text-paper">
      <div class="flex items-center justify-between gap-3 border-b border-white/15 px-5 py-4 font-mono text-[10px] uppercase tracking-wider sm:px-7">
        <span class="flex items-center gap-2"><span class="h-1.5 w-1.5 bg-teal" />{{ locale === 'ru' ? 'Стойка' : 'Rack' }} 01 / 42U</span>
        <div class="flex gap-1" :aria-label="t.rackFace">
          <Button size="sm" variant="ghost" class="h-6 px-2 text-[10px] text-paper hover:bg-white/10 hover:text-white" :class="!rear ? 'bg-white/10' : ''" :aria-pressed="!rear" @click="rear = false">{{ t.rackFront }}</Button>
          <Button size="sm" variant="ghost" class="h-6 px-2 text-[10px] text-paper hover:bg-white/10 hover:text-white" :class="rear ? 'bg-white/10' : ''" :aria-pressed="rear" @click="rear = true">{{ t.rackRear }}</Button>
        </div>
      </div>
      <div class="relative px-5 pb-7 pt-8 sm:px-9 sm:pb-9 sm:pt-10">
        <div class="mb-4 flex justify-between font-mono text-[9px] uppercase tracking-[0.13em] text-white/50"><span>{{ t.rackLabel }}</span><span>01—05</span></div>
        <div class="relative border-x-[12px] border-y-[10px] border-[#343b3c] bg-[#080b0d] px-1 py-3 sm:border-x-[17px]">
          <div class="absolute -left-[10px] bottom-3 top-3 flex flex-col justify-between sm:-left-[14px]" aria-hidden="true"><span v-for="i in 14" :key="i" class="h-[7px] w-[5px] border border-[#59615d] bg-[#080b0d]" /></div>
          <div class="absolute -right-[10px] bottom-3 top-3 flex flex-col justify-between sm:-right-[14px]" aria-hidden="true"><span v-for="i in 14" :key="i" class="h-[7px] w-[5px] border border-[#59615d] bg-[#080b0d]" /></div>
          <div v-for="(item, index) in rows" :key="item.name" :class="index === 2 ? 'mt-10' : index === 4 ? 'mt-8' : 'mt-2'" class="relative">
            <div class="relative overflow-hidden border border-black" :class="item.double ? 'aspect-[7/1]' : 'aspect-[13/1]'">
              <img :src="`/images/${item.face.image}`" :alt="item.name" :style="crop(item.face.uv)" class="absolute max-w-none" width="2048" height="512" />
            </div>
            <div class="mt-1 flex justify-between px-1 font-mono text-[8px] text-white/50"><span>{{ item.name }}</span><span>U{{ item.unit }}</span></div>
          </div>
          <div class="mt-5 flex h-7 items-center justify-center border-y border-white/10 font-mono text-[9px] tracking-[0.3em] text-white/20">CLOUD PROVIDER / {{ locale === 'ru' ? 'ЗАЛ' : 'ROOM' }} 01</div>
        </div>
        <div class="mt-5 flex flex-wrap items-center justify-between gap-2 font-mono text-[9px] uppercase tracking-wider text-white/45"><span>{{ locale === 'ru' ? 'Вычисления / Сеть / Питание' : 'Compute / Network / Power' }}</span><span class="text-[#75a99e]">{{ rear ? t.rackRear : t.rackFront }}</span></div>
      </div>
    </div>
    <figcaption class="mt-3 flex gap-3 text-xs leading-relaxed text-muted-ink"><span class="font-mono text-[10px] text-teal">{{ locale === 'ru' ? 'РИС.' : 'FIG.' }} 01</span><span>{{ t.rackCaption }}</span></figcaption>
  </figure>
</template>
