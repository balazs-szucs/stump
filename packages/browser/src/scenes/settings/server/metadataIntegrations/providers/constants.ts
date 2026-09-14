import { MetadataProvider } from '@stump/graphql'

export const PROVIDER_LABELS: Record<MetadataProvider, string> = {
	[MetadataProvider.Hardcover]: 'Hardcover',
	[MetadataProvider.ComicVine]: 'Comic Vine',
	[MetadataProvider.OpenLibrary]: 'Open Library',
}

export const PROVIDERS = Object.values(MetadataProvider)

export const providerRequiresToken = (provider: MetadataProvider) =>
	provider !== MetadataProvider.OpenLibrary
