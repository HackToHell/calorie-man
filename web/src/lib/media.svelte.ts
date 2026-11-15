/** Reactive "is this a phone-sized screen" flag, matching the CSS breakpoint (900px). */
class Media {
  phone = $state(false);
  constructor() {
    const mq = window.matchMedia('(max-width: 899px)');
    this.phone = mq.matches;
    mq.addEventListener('change', (e) => (this.phone = e.matches));
  }
}
export const media = new Media();
