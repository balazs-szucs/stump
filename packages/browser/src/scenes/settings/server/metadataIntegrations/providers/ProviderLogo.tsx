import { cn } from '@stump/components'
import { MetadataProvider } from '@stump/graphql'

import { PROVIDER_LABELS } from './constants'

type Props = {
	provider: MetadataProvider
	className?: string
}

export function ProviderLogo({ provider, className }: Props) {
	return (
		<img
			src={LOGOS[provider]}
			alt={`${PROVIDER_LABELS[provider]} logo`}
			className={cn('h-16 w-16 object-scale-down', className, {
				'rotate-12 transform': provider === MetadataProvider.Hardcover,
			})}
		/>
	)
}

const LOGOS: Record<MetadataProvider, string> = {
	[MetadataProvider.Hardcover]: '/assets/logos/hardcover.png',
	[MetadataProvider.ComicVine]: '/assets/logos/comicvine.png',
	[MetadataProvider.OpenLibrary]: '/assets/logos/openlibrary.svg',
}
