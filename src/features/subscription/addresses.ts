// Безналоговые штаты США для адреса биллинга. Перенесено из BinCheckerTab без изменений.

export interface TaxFreeAddress {
  stateName: string;
  code: string;
  street: string;
  city: string;
  zip: string;
}

export const TAX_FREE_ADDRESSES: TaxFreeAddress[] = [
  {
    stateName: "Делавэр (0% Tax)",
    code: "DE",
    street: "1201 N Orange St, Ste 700",
    city: "Wilmington",
    zip: "19801",
  },
  {
    stateName: "Орегон (0% Tax)",
    code: "OR",
    street: "1001 SW 5th Ave, Ste 1100",
    city: "Portland",
    zip: "97204",
  },
  {
    stateName: "Нью-Гэмпшир (0% Tax)",
    code: "NH",
    street: "100 Market St, Ste 400",
    city: "Portsmouth",
    zip: "03801",
  },
  {
    stateName: "Монтана (0% Tax)",
    code: "MT",
    street: "208 N Broadway, Ste 320",
    city: "Billings",
    zip: "59101",
  },
  {
    stateName: "Аляска (0% Tax)",
    code: "AK",
    street: "333 W 4th Ave, Ste 200",
    city: "Anchorage",
    zip: "99501",
  },
];
